//! Invocation router
use alloc::{borrow::ToOwned, boxed::Box, format, string::ToString, sync::Arc, vec::Vec};
use core::time::Duration;
use saikuro_core::{
    envelope::{Envelope, InvocationType},
    invocation::InvocationId,
    ResponseEnvelope,
};
use saikuro_event::{ErrorDetail, LogLevel, LogRecord, LogSink, Result, SaikuroError};
use saikuro_exec::{mpsc, oneshot, timeout, ChannelCapacity};

use crate::{
    provider::{Provider, ProviderRegistry},
    stream_state::{ChannelState, DeliveryOutcome, StreamState, StreamStateStore},
    DefaultRouterSink,
};

//  Config

/// Configuration for the invocation router.
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Timeout for call invocations (the router will return a Timeout error if
    /// the provider doesn't respond within this window).
    pub call_timeout: Duration,

    /// Capacity of per-stream item channels.
    pub stream_channel_capacity: ChannelCapacity,

    /// Capacity of per-channel inbound/outbound item channels.
    pub channel_capacity: ChannelCapacity,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            call_timeout: Duration::from_secs(30),
            stream_channel_capacity: ChannelCapacity::DEFAULT,
            channel_capacity: ChannelCapacity::DEFAULT,
        }
    }
}

//  Router

/// The central dispatch hub.
///
/// `InvocationRouter` is cheap to clone :  all state is `Arc`-wrapped inside
/// the registries it references.
pub struct InvocationRouter<S: LogSink + Send + Sync + 'static = DefaultRouterSink> {
    providers: ProviderRegistry,
    streams: StreamStateStore,
    config: RouterConfig,
    /// Sink for `Log`-type envelopes.  Wrapped in `Arc` so `Clone` works.
    log_sink: Arc<S>,
}

impl<S: LogSink + Send + Sync + 'static> Clone for InvocationRouter<S> {
    fn clone(&self) -> Self {
        Self {
            providers: self.providers.clone(),
            streams: self.streams.clone(),
            config: self.config.clone(),
            log_sink: self.log_sink.clone(),
        }
    }
}

impl InvocationRouter<DefaultRouterSink> {
    pub fn new(providers: ProviderRegistry, config: RouterConfig) -> Self {
        Self::with_log_sink(providers, config, default_sink())
    }

    /// Create a router with the given providers and default config.
    pub fn with_providers(providers: ProviderRegistry) -> Self {
        Self::new(providers, RouterConfig::default())
    }
}

impl<S: LogSink + Send + Sync + 'static> InvocationRouter<S> {
    /// Create a router with a custom log sink.
    pub fn with_log_sink<S2: LogSink + Send + Sync + 'static>(
        providers: ProviderRegistry,
        config: RouterConfig,
        sink: S2,
    ) -> InvocationRouter<S2> {
        InvocationRouter {
            providers,
            streams: StreamStateStore::new(),
            config,
            log_sink: Arc::new(sink),
        }
    }

    // State store access
    pub fn streams(&self) -> &StreamStateStore {
        &self.streams
    }

    /// Dispatch an envelope and return the response
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
            InvocationType::Log => self.dispatch_log(envelope).await,
            InvocationType::Announce => {
                // Announce envelopes are handled by the connection layer before
                // reaching the router.  If one leaks through here it is a no-op
                // so we don't panic but we do warn.
                self.log_sink
                    .emit(&LogRecord::new(
                        "",
                        LogLevel::Warn,
                        "saikuro.router",
                        format!(
                            "announce envelope reached router (id={}): should be handled by ConnectionHandler",
                            envelope.id
                        ),
                    ))
                    .await;
                ResponseEnvelope::ok_empty(envelope.id)
            }
        }
    }

    // Call
    async fn dispatch_call(&self, envelope: Envelope) -> ResponseEnvelope {
        let id = envelope.id;

        let provider = match self.resolve_namespace(&envelope.target).await {
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
                self.log_sink
                    .emit(&LogRecord::new(
                        "",
                        LogLevel::Warn,
                        "saikuro.router",
                        format!("provider dropped response sender without replying (id={})", id),
                    ))
                    .await;
                error_response(
                    id,
                    SaikuroError::ProviderUnavailable("response channel dropped".into()).into(),
                )
            }
            Err(_) => {
                self.log_sink
                    .emit(&LogRecord::new(
                        "",
                        LogLevel::Warn,
                        "saikuro.router",
                        format!(
                            "call timed out (id={}, timeout_ms={})",
                            id,
                            self.config.call_timeout.as_millis()
                        ),
                    ))
                    .await;
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

        let provider = match self.resolve_namespace(&envelope.target).await {
            Ok(p) => p,
            Err(e) => return error_response(id, e.into()),
        };

        // Fire-and-forget: we don't wait for any response.
        if let Err(e) = provider.send_invocation(envelope, None).await {
            self.log_sink
                .emit(&LogRecord::new(
                    "",
                    LogLevel::Warn,
                    "saikuro.router",
                    format!("cast dispatch failed (id={}): {e}", id),
                ))
                .await;
            // Still return ok_empty :  the caller opted out of responses.
        }

        ResponseEnvelope::ok_empty(id)
    }

    // Stream
    async fn dispatch_stream_open(&self, envelope: Envelope) -> ResponseEnvelope {
        let id = envelope.id;

        let provider = match self.resolve_namespace(&envelope.target).await {
            Ok(p) => p,
            Err(e) => return error_response(id, e.into()),
        };

        let (item_tx, item_rx) = mpsc::channel(self.config.stream_channel_capacity);
        let state = StreamState::new(item_tx);
        self.streams.insert_stream(id, state, item_rx).await;

        // Send the open request; the provider will start sending items.
        if let Err(e) = provider.send_invocation(envelope, None).await {
            self.streams.remove_stream(&id).await;
            return error_response(id, e.into());
        }

        self.log_sink
            .emit(&LogRecord::new(
                "",
                LogLevel::Debug,
                "saikuro.router",
                format!("stream opened (id={})", id),
            ))
            .await;
        ResponseEnvelope::ok_empty(id)
    }

    // Channel
    async fn dispatch_channel_open(&self, envelope: Envelope) -> ResponseEnvelope {
        let id = envelope.id;

        // If a channel with this id already exists, treat as data frame
        if let Some(channel) = self.streams.get_channel(&id).await {
            // Map the Envelope to a ResponseEnvelope for channel data delivery
            let resp = ResponseEnvelope {
                id,
                ok: true,
                result: envelope.args.first().cloned(),
                error: None,
                seq: envelope.seq,
                stream_control: envelope.stream_control,
            };
            match channel.deliver(resp, true).await {
                DeliveryOutcome::Terminal => {
                    self.streams.remove_channel_if(&id, &channel).await;
                    return ResponseEnvelope::ok_empty(id);
                }
                DeliveryOutcome::Closed => {
                    self.streams.remove_channel_if(&id, &channel).await;
                    return error_response(id, SaikuroError::ChannelClosed.into());
                }
                DeliveryOutcome::OutOfOrder => {
                    self.log_sink
                        .emit(&LogRecord::new(
                            "",
                            LogLevel::Warn,
                            "saikuro.router",
                            format!("out-of-order channel item dropped (id={})", id),
                        ))
                        .await;
                }
                DeliveryOutcome::Delivered => {}
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
        let provider = match self.resolve_namespace(&envelope.target).await {
            Ok(p) => p,
            Err(e) => return error_response(id, e.into()),
        };

        let (inbound_tx, inbound_rx) = mpsc::channel(self.config.channel_capacity);
        let (outbound_tx, outbound_rx) = mpsc::channel(self.config.channel_capacity);
        let state = ChannelState::new(inbound_tx, outbound_tx);
        self.streams
            .insert_channel(id, state, inbound_rx, outbound_rx).await;

        if let Err(e) = provider.send_invocation(envelope, None).await {
            self.streams.remove_channel(&id).await;
            return error_response(id, e.into());
        }

        self.log_sink
            .emit(&LogRecord::new(
                "",
                LogLevel::Debug,
                "saikuro.router",
                format!("channel opened (id={})", id),
            ))
            .await;
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
                response.result.unwrap_or(saikuro_event::Value::Null)
            } else {
                saikuro_event::Value::Null
            });
        }

        ResponseEnvelope::ok(id, saikuro_event::Value::Array(results))
    }

    // Log
    async fn dispatch_log(&self, envelope: Envelope) -> ResponseEnvelope {
        let id = envelope.id;

        // args[0] is the LogRecord as a Value::Map.
        let record = envelope
            .args
            .into_iter()
            .next()
            .and_then(|v| match LogRecord::try_from(v) {
                Ok(r) => Some(r),
                Err(e) => {
                    self.log_sink
                        .emit(&LogRecord::new(
                            "",
                            LogLevel::Warn,
                            "saikuro.router",
                            format!("failed to parse LogRecord from log envelope (id={}): {e}", id),
                        ))
                        .await;
                    None
                }
            });

        match record {
            Some(r) => {
                self.log_sink.emit(&r).await;
            }
            None => {
                self.log_sink
                    .emit(&LogRecord::new(
                        "",
                        LogLevel::Warn,
                        "saikuro.router",
                        format!("log envelope has no valid LogRecord in args[0]; dropping (id={})", id),
                    ))
                    .await;
            }
        }

        ResponseEnvelope::ok_empty(id)
    }

    // Stream item routing
    async fn route_channel_item(&self, response: ResponseEnvelope, inbound: bool) -> Result<()> {
        let id = response.id;
        let state = self
            .streams
            .get_channel(&id).await
            .ok_or_else(|| SaikuroError::ChannelNotFound(id.to_string()))?;

        match state.deliver(response, inbound).await {
            DeliveryOutcome::Closed => {
                self.streams.remove_channel_if(&id, &state).await;
                Err(SaikuroError::ChannelClosed)
            }
            DeliveryOutcome::OutOfOrder => {
                self.log_sink
                    .emit(&LogRecord::new(
                        "",
                        LogLevel::Warn,
                        "saikuro.router",
                        format!("out-of-order channel item dropped (id={})", id),
                    ))
                    .await;
                Ok(())
            }
            DeliveryOutcome::Terminal => {
                self.streams.remove_channel_if(&id, &state).await;
                Ok(())
            }
            DeliveryOutcome::Delivered => Ok(()),
        }
    }

    pub async fn route_channel_inbound(&self, response: ResponseEnvelope) -> Result<()> {
        self.route_channel_item(response, true).await
    }

    /// Route an outbound channel item (provider -> client direction) to the
    /// appropriate open channel's outbound queue.
    pub async fn route_channel_outbound(&self, response: ResponseEnvelope) -> Result<()> {
        self.route_channel_item(response, false).await
    }

    /// Route an inbound stream item to the appropriate open stream.
    pub async fn route_stream_item(&self, response: ResponseEnvelope) -> Result<()> {
        let id = response.id;
        let state = self
            .streams
            .get_stream(&id).await
            .ok_or_else(|| SaikuroError::StreamNotFound(id.to_string()))?;

        match state.deliver(response).await {
            DeliveryOutcome::Closed => {
                self.streams.remove_stream_if(&id, &state).await;
                Err(SaikuroError::StreamClosed)
            }
            DeliveryOutcome::OutOfOrder => {
                self.log_sink
                    .emit(&LogRecord::new(
                        "",
                        LogLevel::Warn,
                        "saikuro.router",
                        format!("out-of-order stream item dropped (id={})", id),
                    ))
                    .await;
                Ok(())
            }
            DeliveryOutcome::Terminal => {
                self.streams.remove_stream_if(&id, &state).await;
                Ok(())
            }
            DeliveryOutcome::Delivered => Ok(()),
        }
    }

    // Helpers
    async fn resolve_namespace(&self, target: &str) -> Result<crate::provider::ProviderHandle> {
        let ns =
            namespace_of(target).ok_or_else(|| SaikuroError::MalformedTarget(target.to_owned()))?;

        let handle = self
            .providers
            .get(ns).await
            .ok_or_else(|| SaikuroError::NoProvider(ns.to_owned()))?;

        if !handle.is_alive() {
            return Err(SaikuroError::ProviderUnavailable(handle.id().to_owned()));
        }

        Ok(handle)
    }
}

//  Helpers
fn namespace_of(target: &str) -> Option<&str> {
    saikuro_core::envelope::split_target(target).map(|(ns, _)| ns)
}

fn error_response(id: InvocationId, detail: ErrorDetail) -> ResponseEnvelope {
    ResponseEnvelope::err(id, detail)
}


fn default_sink() -> DefaultRouterSink {
    #[cfg(feature = "native")]
    {
        saikuro_event::TracingSink
    }
    #[cfg(feature = "wasm")]
    {
        saikuro_event::ConsoleSink
    }
    #[cfg(any(feature = "no_std", feature = "embedded"))]
    {
        saikuro_event::NullSink
    }
}
