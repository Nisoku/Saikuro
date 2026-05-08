//! Invocation router.
//!
//! The router is the central dispatch component.  After the validator has
//! confirmed an envelope is well-formed and permitted, the router:
//!
//! 1. Resolves the target namespace to a provider handle.
//! 2. For `Call`: allocates a one-shot channel, sends work to the provider,
//!    and returns a future that completes when the response arrives.
//! 3. For `Cast`: sends work to the provider and returns immediately.
//! 4. For `Stream`/`Channel`: sets up the state tracking entry, sends the
//!    open request to the provider, and returns the appropriate receiver.
//! 5. For `Batch`: dispatches each item and collects all results.
//! 6. For `Log`: extracts a [`LogRecord`] from `args[0]` and forwards it to
//!    the configured log sink without routing to any provider.

use saikuro_core::{
    envelope::{Envelope, InvocationType, StreamControl},
    error::{ErrorDetail, SaikuroError},
    invocation::InvocationId,
    log::{LogLevel, LogRecord, LogSink},
    ResponseEnvelope,
};
use saikuro_exec::{mpsc, oneshot, timeout};
use std::{sync::Arc, time::Duration};
use tracing::{debug, instrument, warn};

use crate::{
    error::{Result, RouterError},
    provider::{Provider, ProviderRegistry},
    stream_state::{ChannelState, StreamState, StreamStateStore},
};

//  Config

/// Configuration for the invocation router.
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Timeout for call invocations (the router will return a Timeout error if
    /// the provider doesn't respond within this window).
    pub call_timeout: Duration,

    /// Capacity of per-stream item channels.
    pub stream_channel_capacity: usize,

    /// Capacity of per-channel inbound/outbound item channels.
    pub channel_capacity: usize,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            call_timeout: Duration::from_secs(30),
            stream_channel_capacity: 128,
            channel_capacity: 128,
        }
    }
}

//  Tracing-backed default log sink

/// Construct a log sink that forwards [`LogRecord`]s into the `tracing`
/// infrastructure at the matching level.
///
/// The logger `name` and `msg` are concatenated in the tracing event message
/// since `tracing` macros require a literal `target:`.
pub fn tracing_log_sink() -> LogSink {
    Box::new(|record: LogRecord| {
        // tracing macros need a string-literal target; we embed the name in
        // the message instead so callers can still filter by it in log output.
        let line = format!("[{}] {}", record.name, record.msg);
        match record.level {
            LogLevel::Trace => tracing::trace!("{}", line),
            LogLevel::Debug => tracing::debug!("{}", line),
            LogLevel::Info => tracing::info!("{}", line),
            LogLevel::Warn => tracing::warn!("{}", line),
            LogLevel::Error => tracing::error!("{}", line),
        }
    })
}

//  Router

/// The central dispatch hub.
///
/// `InvocationRouter` is cheap to clone :  all state is `Arc`-wrapped inside
/// the registries it references.
#[derive(Clone)]
pub struct InvocationRouter {
    providers: ProviderRegistry,
    streams: StreamStateStore,
    config: RouterConfig,
    /// Sink for `Log`-type envelopes.  Wrapped in `Arc` so `Clone` works.
    log_sink: Arc<LogSink>,
}

impl InvocationRouter {
    pub fn new(providers: ProviderRegistry, config: RouterConfig) -> Self {
        Self::with_log_sink(providers, config, tracing_log_sink())
    }

    /// Create a router with a custom log sink.
    pub fn with_log_sink(providers: ProviderRegistry, config: RouterConfig, sink: LogSink) -> Self {
        Self {
            providers,
            streams: StreamStateStore::new(),
            config,
            log_sink: Arc::new(sink),
        }
    }

    /// Create a router with default config.
    pub fn with_providers(providers: ProviderRegistry) -> Self {
        Self::new(providers, RouterConfig::default())
    }

    // State store access

    /// Access the shared [`StreamStateStore`] directly.
    ///
    /// Primarily useful in tests and the runtime server loop when it needs to
    /// take receivers to forward stream/channel items to the connected adapter.
    pub fn streams(&self) -> &StreamStateStore {
        &self.streams
    }

    // Public dispatch API

    /// Dispatch an envelope and return the response.
    ///
    /// For `Cast` the response is always `ResponseEnvelope::ok_empty`.
    /// For `Stream` / `Channel` the response carries the stream ID; items
    /// arrive on the returned channel.
    /// For `Log` the log record is forwarded to the sink and
    /// `ResponseEnvelope::ok_empty` is returned (no provider is involved).
    #[instrument(skip(self, envelope), fields(
        id = %envelope.id,
        target = %envelope.target,
        invocation_type = %envelope.invocation_type,
    ))]
    pub async fn dispatch(&self, envelope: Envelope) -> ResponseEnvelope {
        match envelope.invocation_type {
            InvocationType::Call => self.dispatch_call(envelope).await,
            InvocationType::Cast => self.dispatch_cast(envelope).await,
            InvocationType::Stream => self.dispatch_stream_open(envelope).await,
            InvocationType::Channel => self.dispatch_channel_open(envelope).await,
            InvocationType::Batch => self.dispatch_batch(envelope).await,
            InvocationType::Resource => {
                // Resource handles are provider-specific; route the same way
                // as a call and let the provider interpret the args.
                self.dispatch_call(envelope).await
            }
            InvocationType::Log => self.dispatch_log(envelope),
            InvocationType::Announce => {
                // Announce envelopes are handled by the connection layer before
                // reaching the router.  If one leaks through here it is a no-op
                // so we don't panic but we do warn.
                warn!(id = %envelope.id, "announce envelope reached router :  should be handled by ConnectionHandler");
                ResponseEnvelope::ok_empty(envelope.id)
            }
        }
    }

    // Call

    async fn dispatch_call(&self, envelope: Envelope) -> ResponseEnvelope {
        let id = envelope.id;

        let provider = match self.resolve_namespace(&envelope.target) {
            Ok(p) => p,
            Err(e) => return error_response(id, e.into()),
        };

        let (resp_tx, resp_rx) = oneshot::channel();

        if let Err(e) = provider.send_invocation(envelope, Some(resp_tx)).await {
            return error_response(id, e.into());
        }

        match timeout(self.config.call_timeout, resp_rx).await {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => {
                warn!(%id, "provider dropped response sender without replying");
                error_response(
                    id,
                    SaikuroError::ProviderUnavailable("response channel dropped".into()).into(),
                )
            }
            Err(_) => {
                warn!(%id, timeout_ms = self.config.call_timeout.as_millis(), "call timed out");
                error_response(
                    id,
                    SaikuroError::Timeout {
                        millis: self.config.call_timeout.as_millis() as u64,
                    }
                    .into(),
                )
            }
        }
    }

    // Cast

    async fn dispatch_cast(&self, envelope: Envelope) -> ResponseEnvelope {
        let id = envelope.id;

        let provider = match self.resolve_namespace(&envelope.target) {
            Ok(p) => p,
            Err(e) => return error_response(id, e.into()),
        };

        // Fire-and-forget: we don't wait for any response.
        if let Err(e) = provider.send_invocation(envelope, None).await {
            warn!(%id, "cast dispatch failed: {e}");
            // Still return ok_empty :  the caller opted out of responses.
        }

        ResponseEnvelope::ok_empty(id)
    }

    // Stream

    /// Open a server-to-client stream.
    ///
    /// Returns an `ok_empty` response immediately; items arrive on the
    /// `mpsc::Receiver<ResponseEnvelope>` that callers subscribe to via the
    /// runtime's stream subscription API.
    async fn dispatch_stream_open(&self, envelope: Envelope) -> ResponseEnvelope {
        let id = envelope.id;

        let provider = match self.resolve_namespace(&envelope.target) {
            Ok(p) => p,
            Err(e) => return error_response(id, e.into()),
        };

        let (item_tx, item_rx) = mpsc::channel(self.config.stream_channel_capacity);
        let state = StreamState::new(item_tx);
        self.streams.insert_stream(id, state, item_rx);

        // Send the open request; the provider will start sending items.
        if let Err(e) = provider.send_invocation(envelope, None).await {
            self.streams.remove_stream(&id);
            return error_response(id, e.into());
        }

        debug!(%id, "stream opened");
        ResponseEnvelope::ok_empty(id)
    }

    // Channel

    async fn dispatch_channel_open(&self, envelope: Envelope) -> ResponseEnvelope {
        let id = envelope.id;

        // If a channel with this id already exists, treat as data frame
        if let Some(channel) = self.streams.get_channel(&id) {
            // Map the Envelope to a ResponseEnvelope for channel data delivery
            let resp = ResponseEnvelope {
                id,
                ok: true,
                result: envelope.args.first().cloned(),
                error: None,
                seq: envelope.seq,
                stream_control: envelope.stream_control,
            };
            if let Err(e) = channel.inbound_tx.send(resp).await {
                return error_response(
                    id,
                    SaikuroError::ProviderUnavailable(format!("channel data delivery failed: {e}"))
                        .into(),
                );
            }
            // If this is a terminal frame, clean up the channel and return ok_empty
            if matches!(
                envelope.stream_control,
                Some(StreamControl::End | StreamControl::Abort)
            ) {
                self.streams.remove_channel(&id);
                return ResponseEnvelope::ok_empty(id);
            }
            // For non-terminal frames, do not return a response (one-way)
            return ResponseEnvelope {
                id,
                ok: true,
                result: None,
                error: None,
                seq: None,
                stream_control: None,
            };
        }

        // Otherwise, open a new channel as before
        let provider = match self.resolve_namespace(&envelope.target) {
            Ok(p) => p,
            Err(e) => return error_response(id, e.into()),
        };

        let (inbound_tx, inbound_rx) = mpsc::channel(self.config.channel_capacity);
        let (outbound_tx, outbound_rx) = mpsc::channel(self.config.channel_capacity);
        let state = ChannelState::new(inbound_tx, outbound_tx);
        self.streams
            .insert_channel(id, state, inbound_rx, outbound_rx);

        if let Err(e) = provider.send_invocation(envelope, None).await {
            self.streams.remove_channel(&id);
            return error_response(id, e.into());
        }

        debug!(%id, "channel opened");
        ResponseEnvelope::ok_empty(id)
    }

    // Batch

    async fn dispatch_batch(&self, envelope: Envelope) -> ResponseEnvelope {
        let id = envelope.id;
        let items = match envelope.batch_items {
            Some(items) => items,
            None => {
                return error_response(
                    id,
                    SaikuroError::MalformedEnvelope("batch has no items".into()).into(),
                );
            }
        };

        let mut results = Vec::with_capacity(items.len());
        for item in items {
            let response = Box::pin(self.dispatch(item)).await;
            // Represent each sub-response as its result value (or Null on error).
            results.push(if response.ok {
                response.result.unwrap_or(saikuro_core::value::Value::Null)
            } else {
                saikuro_core::value::Value::Null
            });
        }

        ResponseEnvelope::ok(id, saikuro_core::value::Value::Array(results))
    }

    // Log

    /// Handle a `Log`-type envelope.
    ///
    /// Extracts the [`LogRecord`] from `args[0]`, forwards it to the log sink,
    /// and returns `ok_empty`.  Never touches a provider.
    fn dispatch_log(&self, envelope: Envelope) -> ResponseEnvelope {
        let id = envelope.id;

        // args[0] must be the serialised LogRecord (a map value).
        let record = envelope.args.into_iter().next().and_then(|v| {
            // The value arrives as a MessagePack map; deserialise it.
            let bytes = rmp_serde::to_vec_named(&v).ok()?;
            rmp_serde::from_slice::<LogRecord>(&bytes).ok()
        });

        match record {
            Some(r) => {
                (self.log_sink)(r);
            }
            None => {
                warn!(%id, "log envelope has no valid LogRecord in args[0]; dropping");
            }
        }

        ResponseEnvelope::ok_empty(id)
    }

    // Stream item routing

    /// Route an inbound channel item (client -> provider direction) to the
    /// appropriate open channel's inbound queue.
    ///
    /// This is called when the client sends a follow-up message on an already-
    /// opened channel (i.e. a `Channel`-type envelope whose ID matches an
    /// existing channel state entry).
    pub async fn route_channel_inbound(&self, response: ResponseEnvelope) -> Result<()> {
        let id = response.id;
        let state = self
            .streams
            .get_channel(&id)
            .ok_or_else(|| RouterError::ChannelNotFound(id.to_string()))?;

        if state.is_closed() {
            return Err(RouterError::ChannelClosed(id.to_string()));
        }

        let is_terminal = matches!(
            response.stream_control,
            Some(StreamControl::End) | Some(StreamControl::Abort)
        );

        state
            .inbound_tx
            .send(response)
            .await
            .map_err(|_| RouterError::ChannelClosed(id.to_string()))?;

        if is_terminal {
            state.mark_closed();
            self.streams.remove_channel(&id);
        }

        Ok(())
    }

    /// Route an outbound channel item (provider -> client direction) to the
    /// appropriate open channel's outbound queue.
    ///
    /// Called by the provider adapter when it wants to push a message to the
    /// client side of an open channel.
    pub async fn route_channel_outbound(&self, response: ResponseEnvelope) -> Result<()> {
        let id = response.id;
        let state = self
            .streams
            .get_channel(&id)
            .ok_or_else(|| RouterError::ChannelNotFound(id.to_string()))?;

        if state.is_closed() {
            return Err(RouterError::ChannelClosed(id.to_string()));
        }

        let is_terminal = matches!(
            response.stream_control,
            Some(StreamControl::End) | Some(StreamControl::Abort)
        );

        state
            .outbound_tx
            .send(response)
            .await
            .map_err(|_| RouterError::ChannelClosed(id.to_string()))?;

        if is_terminal {
            state.mark_closed();
            self.streams.remove_channel(&id);
        }

        Ok(())
    }

    /// Route an inbound stream item to the appropriate open stream.
    pub async fn route_stream_item(&self, response: ResponseEnvelope) -> Result<()> {
        let id = response.id;
        let state = self
            .streams
            .get_stream(&id)
            .ok_or_else(|| RouterError::StreamNotFound(id.to_string()))?;

        if state.is_closed() {
            return Err(RouterError::StreamClosed(id.to_string()));
        }

        // Sequence check.
        if let Some(seq) = response.seq {
            if !state.advance_seq(seq) {
                warn!(%id, seq, "out-of-order stream item dropped");
            }
        }

        // Determine if this is a terminal frame before consuming `response`.
        let is_terminal = matches!(
            response.stream_control,
            Some(StreamControl::End) | Some(StreamControl::Abort)
        );

        // Send the item first so the receiver is still alive when we deliver.
        state
            .item_tx
            .send(response)
            .await
            .map_err(|_| RouterError::StreamClosed(id.to_string()))?;

        // Only after successful delivery, mark closed and drop the receiver.
        if is_terminal {
            state.mark_closed();
            self.streams.remove_stream(&id);
        }

        Ok(())
    }

    // Helpers

    fn resolve_namespace(&self, target: &str) -> Result<crate::provider::ProviderHandle> {
        let ns =
            namespace_of(target).ok_or_else(|| RouterError::MalformedTarget(target.to_owned()))?;

        let handle = self
            .providers
            .get(ns)
            .ok_or_else(|| RouterError::NoProvider(ns.to_owned()))?;

        if !handle.is_alive() {
            return Err(RouterError::ProviderUnavailable(handle.id().to_owned()));
        }

        Ok(handle)
    }
}

//  Helpers

fn namespace_of(target: &str) -> Option<&str> {
    let dot = target.rfind('.')?;
    Some(&target[..dot])
}

fn error_response(id: InvocationId, detail: ErrorDetail) -> ResponseEnvelope {
    ResponseEnvelope::err(id, detail)
}

// Allow RouterError to convert into ErrorDetail
impl From<RouterError> for ErrorDetail {
    fn from(err: RouterError) -> Self {
        let code = match &err {
            RouterError::NoProvider(_) => saikuro_core::error::ErrorCode::NoProvider,
            RouterError::ProviderUnavailable(_) => {
                saikuro_core::error::ErrorCode::ProviderUnavailable
            }
            RouterError::MalformedTarget(_) => saikuro_core::error::ErrorCode::MalformedEnvelope,
            RouterError::StreamNotFound(_) | RouterError::ChannelNotFound(_) => {
                saikuro_core::error::ErrorCode::StreamClosed
            }
            RouterError::StreamClosed(_) => saikuro_core::error::ErrorCode::StreamClosed,
            RouterError::ChannelClosed(_) => saikuro_core::error::ErrorCode::ChannelClosed,
            RouterError::BatchItemFailed { .. } => saikuro_core::error::ErrorCode::ProviderError,
            RouterError::SendError(_) => saikuro_core::error::ErrorCode::ProviderUnavailable,
        };
        ErrorDetail::new(code, err.to_string())
    }
}
