use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic_util::Arc;
#[cfg(target_has_atomic = "ptr")]
use saikuro_core::Arc;

use bytes::Bytes;
use futures::future::FutureExt;
use saikuro_core::{
    capability::CapabilitySet,
    envelope::{classify_frame, Envelope, FrameKind, InvocationType},
    invocation::InvocationId,
    schema::{Schema, Visibility},
    RegistrationToken, ResponseEnvelope,
};
use saikuro_event::{ErrorDetail, LogLevel, LogRecord, LogSink, Value};
use saikuro_exec::{mpsc, oneshot, spawn};
use saikuro_router::{
    provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem},
    router::InvocationRouter,
};
use saikuro_schema::{
    capability_engine::{CapabilityEngine, CapabilityOutcome},
    registry::SchemaRegistry,
    validator::InvocationValidator,
};
use serde::{Deserialize, Serialize};
use spin::Mutex;

use crate::transport_adapter::{RuntimeReceiver, RuntimeSender};

//  Pending call map

/// Tracks in-flight `Call` invocations forwarded to a wire-connected provider.
/// Maps `InvocationId -> oneshot::Sender<ResponseEnvelope>`.
type PendingCalls = Arc<Mutex<BTreeMap<InvocationId, oneshot::Sender<ResponseEnvelope>>>>;

/// Capacity of the per-connection wire queues (outbound forward frames and
/// the wire-provider work queue).
#[cfg(saikuro_test_capacity = "small")]
const WIRE_CHANNEL_CAPACITY: saikuro_exec::ChannelCapacity =
    match saikuro_exec::ChannelCapacity::new(4) {
        Ok(cap) => cap,
        Err(_) => panic!(),
    };
#[cfg(not(saikuro_test_capacity = "small"))]
const WIRE_CHANNEL_CAPACITY: saikuro_exec::ChannelCapacity = saikuro_exec::ChannelCapacity::MAX;

/// Encode a serializable value as MessagePack `Bytes`.
fn encode_bytes<T: Serialize>(value: &T) -> Result<Bytes, String> {
    saikuro_core::msgpack::to_vec(value)
        .map(Bytes::from)
        .map_err(|e| e.to_string())
}

/// The shared, process-wide empty peer-capability set.
pub fn empty_peer_capabilities() -> saikuro_core::Arc<CapabilitySet> {
    static EMPTY: spin::Mutex<Option<saikuro_core::Arc<CapabilitySet>>> = spin::Mutex::new(None);
    let mut guard = EMPTY.lock();
    if guard.is_none() {
        *guard = Some(saikuro_core::Arc::new(CapabilitySet::empty()));
    }
    guard
        .as_ref()
        .expect("empty caps initialized above")
        .clone()
}

/// A handler for a single connected peer.
///
/// Generic over the transport halves so it works with every backend and
/// compiles cleanly on wasm32 targets.
pub struct ConnectionHandler<S, R>
where
    S: RuntimeSender + 'static,
    R: RuntimeReceiver + 'static,
{
    pub peer_id: String,
    /// Identity of this connection's provider registration.
    pub registration_token: RegistrationToken,
    pub sender: S,
    pub receiver: R,
    pub validator: InvocationValidator,
    pub capability_engine: CapabilityEngine,
    pub router: InvocationRouter,
    /// Capabilities granted to this peer.
    pub peer_capabilities: saikuro_core::Arc<CapabilitySet>,
    pub max_message_size: usize,
    /// Schema registry shared with the runtime; used to merge announced schemas.
    pub schema_registry: SchemaRegistry,
    /// Provider registry shared with the runtime; used to register/deregister
    /// wire-forwarding provider handles when the peer announces its schema.
    pub provider_registry: ProviderRegistry,
    /// Log sink for structured logging.
    pub log: Arc<dyn LogSink>,
}

impl<S, R> ConnectionHandler<S, R>
where
    S: RuntimeSender + 'static,
    R: RuntimeReceiver + 'static,
{
    /// Build a handler in **sandbox mode**.
    pub fn sandboxed(mut self) -> Self {
        self.capability_engine = CapabilityEngine::sandboxed();
        self
    }

    /// Return `true` if this handler is operating in sandbox mode.
    pub fn is_sandboxed(&self) -> bool {
        self.capability_engine.is_sandboxed()
    }
}

impl<S, R> ConnectionHandler<S, R>
where
    S: RuntimeSender + 'static,
    R: RuntimeReceiver + 'static,
{
    /// Run the receive loop until the connection is closed or an unrecoverable
    /// error occurs.
    pub async fn run(mut self: Box<Self>) {
        {
            let mut record = LogRecord::now(
                LogLevel::Info,
                "saikuro.runtime.connection",
                "connection established",
            );
            record.set_context("peer", self.peer_id.clone());
            self.log.emit(&record).await;
        }

        // Shared pending-call map: ForwardTask writes response_tx into this;
        // the recv loop reads it when a ResponseEnvelope arrives from the peer.
        let pending: PendingCalls = Arc::new(Mutex::new(BTreeMap::new()));

        // Channel through which the ForwardTask sends frames TO the peer.
        // The recv loop serialises all outbound writes through `self.sender`.
        let (forward_tx, mut forward_rx) = mpsc::channel::<Bytes>(WIRE_CHANNEL_CAPACITY);

        loop {
            saikuro_exec::select! {
                // Outbound frame from the ForwardTask (call forwarded to this
                // peer acting as a provider).
                frame_opt = forward_rx.recv() => {
                    match frame_opt {
                        Some(frame) => {
                            if let Err(e) = self.sender.send(frame).await {
                                let mut record = LogRecord::now(
                                    LogLevel::Error,
                                    "saikuro.runtime.connection",
                                    "send error on forwarded call",
                                );
                                record.set_context("peer", self.peer_id.clone());
                                record.set_context("error", alloc::format!("{e}"));
                                self.log.emit(&record).await;
                                break;
                            }
                        }
                        None => {
                            let mut record = LogRecord::now(
                                LogLevel::Info,
                                "saikuro.runtime.connection",
                                "forward channel closed",
                            );
                            record.set_context("peer", self.peer_id.clone());
                            self.log.emit(&record).await;
                            break;
                        }
                    }
                }

                // Inbound frame from the peer (either a new request OR a
                // response to a previously forwarded call).
                incoming = self.receiver.recv().fuse() => {
                    match incoming {
                        Ok(Some(frame)) => {
                            if !self.handle_incoming(frame, &pending, &forward_tx).await {
                                break;
                            }
                        }
                        Ok(None) => {
                            let mut record = LogRecord::now(
                                LogLevel::Info,
                                "saikuro.runtime.connection",
                                "connection closed by peer",
                            );
                            record.set_context("peer", self.peer_id.clone());
                            self.log.emit(&record).await;
                            break;
                        }
                        Err(e) => {
                            let mut record = LogRecord::now(
                                LogLevel::Error,
                                "saikuro.runtime.connection",
                                "recv error",
                            );
                            record.set_context("peer", self.peer_id.clone());
                            record.set_context("error", alloc::format!("{e}"));
                            self.log.emit(&record).await;
                            break;
                        }
                    }
                }
            }
        }

        // Clean up: deregister any provider the peer announced.
        self.provider_registry
            .deregister(&self.peer_id, self.registration_token)
            .await;
        self.schema_registry
            .deregister_provider(&self.peer_id, self.registration_token)
            .await;

        {
            let mut record = LogRecord::now(
                LogLevel::Info,
                "saikuro.runtime.connection",
                "connection handler exiting",
            );
            record.set_context("peer", self.peer_id.clone());
            self.log.emit(&record).await;
        }
    }

    /// Decode, validate, check capabilities, and route a single frame.
    ///
    /// Returns `(response, Option<filtered_schema>)`.  The second element is
    /// `Some` only when sandbox mode is active and the frame was a successful
    /// `Announce`:  in that case the caller must push the schema back to the
    /// peer.
    async fn handle_frame(
        &self,
        frame: Bytes,
        pending: &PendingCalls,
        forward_tx: &mpsc::Sender<Bytes>,
    ) -> Option<(ResponseEnvelope, Option<Box<Schema>>)> {
        // Announce frames carry the schema in `args[0]`; decode it as a
        // typed `Schema` instead of serde's untagged `Value`.
        if classify_frame(&frame) == FrameKind::Announce {
            let envelope = match self.decode_envelope::<Schema>(&frame).await {
                Ok(e) => e,
                Err(Some(resp)) => return Some((*resp, None)),
                Err(None) => return None,
            };
            let id = envelope.id;
            self.log_received(&envelope.target, id).await;
            let response = self.handle_announce(envelope, pending, forward_tx).await;
            // If sandbox mode is on and the announce succeeded, build the
            // filtered schema to push back to the peer.
            let sandbox_schema = if self.capability_engine.is_sandboxed() && response.ok {
                self.build_filtered_schema().await
            } else {
                None
            };
            return Some((response, sandbox_schema));
        }

        // 1. Decode the MessagePack envelope.
        let envelope = match self.decode_envelope(&frame).await {
            Ok(e) => e,
            Err(Some(resp)) => return Some((*resp, None)),
            Err(None) => return None,
        };

        let id = envelope.id;
        self.log_received(&envelope.target, id).await;

        // 2. Handle system envelopes before schema validation. Announce frames
        // are handled in the typed branch above; one that escapes
        // classification degrades to validation against an unknown
        // `$saikuro.announce` target and is rejected there.
        match envelope.invocation_type {
            InvocationType::Log => {
                // Let the router's log sink handle it:  no validation needed.
                return Some((self.router.dispatch_with(envelope, Some(frame)).await, None));
            }
            _ => {}
        }

        // 3. Validate the envelope against the schema.
        let validation = match self.validator.validate(&envelope).await {
            Ok(report) => report,
            Err(e) => {
                return Some((
                    ResponseEnvelope::err(id, ErrorDetail::new(e.error_code(), e.to_string())),
                    None,
                ));
            }
        };

        // 4. Capability check.
        match self
            .capability_engine
            .check_ref(&self.peer_capabilities, &validation.function_ref)
        {
            CapabilityOutcome::Granted => {}
            CapabilityOutcome::Denied { missing } => {
                return Some((
                    ResponseEnvelope::err(
                        id,
                        ErrorDetail::new(
                            saikuro_event::ErrorCode::CapabilityDenied,
                            format!("caller lacks '{}' to invoke '{}'", missing, envelope.target),
                        ),
                    ),
                    None,
                ));
            }
        }

        // 5. Route to provider.  The raw frame travels with the invocation so
        // a wire provider forwards it verbatim instead of re-encoding.
        Some((self.router.dispatch_with(envelope, Some(frame)).await, None))
    }

    /// Decode a MessagePack frame into an [`Envelope`], or return an error
    /// response on failure.
    async fn decode_envelope<A>(
        &self,
        frame: &[u8],
    ) -> Result<Envelope<A>, Option<Box<ResponseEnvelope>>>
    where
        A: for<'de> Deserialize<'de>,
    {
        match saikuro_core::msgpack::from_slice::<Envelope<A>>(frame) {
            Ok(env) => Ok(env),
            Err(e) => {
                let mut record = LogRecord::now(
                    LogLevel::Warn,
                    "saikuro.runtime.connection",
                    "envelope decode failed",
                );
                record.set_context("peer", self.peer_id.clone());
                record.set_context("error", alloc::format!("{e}"));
                self.log.emit(&record).await;
                let id = match InvocationId::new() {
                    Ok(id) => id,
                    Err(_error) => {
                        let mut record = LogRecord::now(
                            LogLevel::Error,
                            "saikuro.runtime.connection",
                            "cannot generate malformed-envelope response ID",
                        );
                        record.set_context("peer", self.peer_id.clone());
                        self.log.emit(&record).await;
                        return Err(None);
                    }
                };
                Err(Some(Box::new(ResponseEnvelope::err(
                    id,
                    ErrorDetail::new(
                        saikuro_event::ErrorCode::MalformedEnvelope,
                        format!("msgpack decode error: {e}"),
                    ),
                ))))
            }
        }
    }

    /// Emit a debug record for an incoming envelope.
    async fn log_received(&self, target: &str, id: InvocationId) {
        let mut record = LogRecord::now(
            LogLevel::Debug,
            "saikuro.runtime.connection",
            "received envelope",
        );
        record.set_context("peer", self.peer_id.clone());
        record.set_context("id", alloc::format!("{}", id));
        record.set_context("target", target.to_owned());
        self.log.emit(&record).await;
    }

    /// Process one incoming frame. Returns `false` when the loop should break.
    async fn handle_incoming(
        &mut self,
        frame: Bytes,
        pending: &PendingCalls,
        forward_tx: &mpsc::Sender<Bytes>,
    ) -> bool {
        if frame.len() > self.max_message_size {
            let err = ErrorDetail::new(
                saikuro_event::ErrorCode::MessageTooLarge,
                format!(
                    "frame {} bytes exceeds limit {} bytes",
                    frame.len(),
                    self.max_message_size
                ),
            );
            let id = match InvocationId::new() {
                Ok(id) => id,
                Err(_error) => {
                    let mut record = LogRecord::now(
                        LogLevel::Error,
                        "saikuro.runtime.connection",
                        "cannot generate oversized-frame response ID",
                    );
                    record.set_context("peer", self.peer_id.clone());
                    self.log.emit(&record).await;
                    return false;
                }
            };
            let response = ResponseEnvelope::err(id, err);
            let _ = self.send_response(response).await;
            return true;
        }

        // Responses carry an `ok` key and requests carry `version`/`type`;
        // classify by scanning the top-level map keys in a single pass
        if classify_frame(&frame) == FrameKind::Response {
            if let Ok(resp) = saikuro_core::msgpack::from_slice::<ResponseEnvelope>(&frame) {
                if let Some(sender) = pending.lock().remove(&resp.id) {
                    let _ = sender.send(resp);
                    return true;
                }
            }
        }

        let Some((response, sandbox_schema)) = self.handle_frame(frame, pending, forward_tx).await
        else {
            return false;
        };

        if let Err(e) = self.send_response(response).await {
            let mut record =
                LogRecord::now(LogLevel::Error, "saikuro.runtime.connection", "send error");
            record.set_context("peer", self.peer_id.clone());
            record.set_context("error", e.to_string());
            self.log.emit(&record).await;
            return false;
        }

        if let Some(filtered) = sandbox_schema {
            if let Err(e) = self.push_sandbox_schema(filtered).await {
                let mut record = LogRecord::now(
                    LogLevel::Error,
                    "saikuro.runtime.connection",
                    "failed to push sandbox schema",
                );
                record.set_context("peer", self.peer_id.clone());
                record.set_context("error", e.to_string());
                self.log.emit(&record).await;
                return false;
            }
        }

        true
    }

    /// Handle a schema-announcement envelope.
    async fn handle_announce(
        &self,
        envelope: Envelope<Schema>,
        pending: &PendingCalls,
        forward_tx: &mpsc::Sender<Bytes>,
    ) -> ResponseEnvelope {
        let id = envelope.id;

        // Announced immediately after connecting, `args[0]` must be a schema.
        // Unlike other paths, announce frames are decoded typed: `handle_frame`
        // routes them through [`Envelope<Schema>`], so the value arrives as a
        // `Schema` directly instead of a Value->bytes->Schema round trip.
        let schema: Option<Schema> = envelope.args.into_iter().next();

        match schema {
            Some(s) => {
                let ns_count = s.namespaces.len();
                let namespaces: Vec<String> = s.namespaces.keys().cloned().collect();

                match self
                    .schema_registry
                    .merge_schema_with_token(s, &self.peer_id, self.registration_token)
                    .await
                {
                    Ok(()) => {
                        {
                            let mut record = LogRecord::now(
                                LogLevel::Info,
                                "saikuro.runtime.connection",
                                "schema announced and merged",
                            );
                            record.set_context("peer", self.peer_id.clone());
                            record.set_context("namespaces", alloc::format!("{ns_count}"));
                            self.log.emit(&record).await;
                        }

                        // Register a wire-forwarding provider handle so the
                        // router can dispatch calls to this peer.
                        self.register_wire_provider(namespaces, pending, forward_tx)
                            .await;

                        ResponseEnvelope::ok_empty(id)
                    }
                    Err(e) => {
                        let mut record = LogRecord::now(
                            LogLevel::Warn,
                            "saikuro.runtime.connection",
                            "schema merge failed",
                        );
                        record.set_context("peer", self.peer_id.clone());
                        record.set_context("error", alloc::format!("{e}"));
                        self.log.emit(&record).await;
                        ResponseEnvelope::err(
                            id,
                            ErrorDetail::new(
                                saikuro_event::ErrorCode::Internal,
                                format!("schema merge error: {e}"),
                            ),
                        )
                    }
                }
            }
            None => {
                let mut record = LogRecord::now(
                    LogLevel::Warn,
                    "saikuro.runtime.connection",
                    "announce envelope has no valid Schema in args[0]",
                );
                record.set_context("peer", self.peer_id.clone());
                self.log.emit(&record).await;
                ResponseEnvelope::err(
                    id,
                    ErrorDetail::new(
                        saikuro_event::ErrorCode::MalformedEnvelope,
                        "announce envelope must carry a Schema in args[0]".to_owned(),
                    ),
                )
            }
        }
    }

    /// Create and register a [`ProviderHandle`] that forwards invocations to
    /// the connected peer over the wire.
    async fn register_wire_provider(
        &self,
        namespaces: Vec<String>,
        pending: &PendingCalls,
        forward_tx: &mpsc::Sender<Bytes>,
    ) {
        let (work_tx, mut work_rx) = mpsc::channel::<ProviderWorkItem>(WIRE_CHANNEL_CAPACITY);
        let handle = ProviderHandle::with_registration_token(
            self.peer_id.clone(),
            self.registration_token,
            namespaces,
            work_tx,
        );
        self.provider_registry.register(handle).await;

        let pending_clone = pending.clone();
        let forward_tx_clone = forward_tx.clone();
        let peer_id = self.peer_id.clone();
        let log = self.log.clone();

        spawn(async move {
            while let Some(item) = work_rx.recv().await {
                let frame = match item.raw {
                    Some(raw) => raw,
                    None => match encode_bytes(&item.envelope) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            let mut record = LogRecord::now(
                                LogLevel::Warn,
                                "saikuro.runtime.connection",
                                "failed to encode forwarded call",
                            );
                            record.set_context("peer", peer_id.clone());
                            record.set_context("error", e.to_string());
                            log.emit(&record).await;
                            if let Some(tx) = item.response_tx {
                                let _ = tx.send(ResponseEnvelope::err(
                                    item.envelope.id,
                                    ErrorDetail::new(
                                        saikuro_event::ErrorCode::Internal,
                                        format!("encode error: {e}"),
                                    ),
                                ));
                            }
                            continue;
                        }
                    },
                };

                // Register before sending so an incoming response can find the
                // pending entry; remove it if the send fails to preserve the
                // original orphan-prevention behavior.
                if let Some(resp_tx) = item.response_tx {
                    pending_clone.lock().insert(item.envelope.id, resp_tx);
                }

                // Send the frame to the peer (via the connection handler's sender).
                if forward_tx_clone.send(frame).await.is_err() {
                    let mut record = LogRecord::now(
                        LogLevel::Warn,
                        "saikuro.runtime.connection",
                        "forward channel closed; provider disconnected",
                    );
                    record.set_context("peer", peer_id.clone());
                    log.emit(&record).await;
                    pending_clone.lock().remove(&item.envelope.id);
                    break;
                }
            }
            let mut record = LogRecord::now(
                LogLevel::Debug,
                "saikuro.runtime.connection",
                "wire-forward task exiting",
            );
            record.set_context("peer", peer_id.clone());
            log.emit(&record).await;
        });
    }

    /// Build a capability-filtered schema snapshot for a sandboxed peer.
    ///
    /// Only namespaces and functions visible to `peer_capabilities` (and not
    /// `Internal` or `Private`) are included.
    async fn build_filtered_schema(&self) -> Option<Box<Schema>> {
        match self
            .schema_registry
            .snapshot_filtered(|_ns_name, _fn_name, schema| {
                // Private functions are never visible; `check` additionally
                // rejects `Internal` functions in sandbox mode.
                schema.visibility != Visibility::Private
                    && matches!(
                        self.capability_engine
                            .check(&self.peer_capabilities, schema),
                        CapabilityOutcome::Granted
                    )
            })
            .await
        {
            Ok(schema) => Some(Box::new(schema)),
            Err(e) => {
                let mut record = LogRecord::now(
                    LogLevel::Error,
                    "saikuro.runtime.connection",
                    "filtered schema snapshot capacity exceeded",
                );
                record.set_context("peer", self.peer_id.clone());
                record.set_context("error", alloc::format!("{}", e));
                self.log.emit(&record).await;
                None
            }
        }
    }

    /// Encode `filtered_schema` as a `Value` and push it as an unsolicited
    /// `Announce` frame to the peer.  The peer uses this to discover what it
    /// is allowed to call.
    async fn push_sandbox_schema(&mut self, filtered: Box<Schema>) -> Result<(), String> {
        let schema_value: Value = {
            let bytes =
                encode_bytes(&filtered).map_err(|e| format!("sandbox schema encode error: {e}"))?;
            saikuro_core::msgpack::from_slice::<Value>(&bytes)
                .map_err(|e| format!("sandbox schema value decode error: {e}"))?
        };
        let announce = Envelope::announce(schema_value)
            .map_err(|e| format!("announce invocation ID error: {e}"))?;
        let frame =
            encode_bytes(&announce).map_err(|e| format!("announce frame encode error: {e}"))?;
        {
            let mut record = LogRecord::now(
                LogLevel::Info,
                "saikuro.runtime.connection",
                "pushing sandbox-filtered schema to peer",
            );
            record.set_context("peer", self.peer_id.clone());
            self.log.emit(&record).await;
        }
        self.sender.send(frame).await.map_err(|e| e.to_string())
    }

    async fn send_response(&mut self, response: ResponseEnvelope) -> Result<(), String> {
        let frame = encode_bytes(&response).map_err(|e| format!("response encode error: {e}"))?;
        self.sender.send(frame).await.map_err(|e| e.to_string())
    }
}
