use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use bytes::Bytes;
use futures::future::FutureExt;
use saikuro_core::{
    capability::CapabilitySet,
    envelope::{Envelope, InvocationType},
    invocation::InvocationId,
    schema::Schema,
    RegistrationToken, ResponseEnvelope,
};
use saikuro_event::{ErrorDetail, Value};
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
use serde::Serialize;
use spin::Mutex;
use tracing::{debug, error, info, instrument, warn};

use crate::transport_adapter::{RuntimeReceiver, RuntimeSender};

//  Pending call map

/// Tracks in-flight `Call` invocations forwarded to a wire-connected provider.
/// Maps `InvocationId -> oneshot::Sender<ResponseEnvelope>`.
type PendingCalls = Arc<Mutex<BTreeMap<InvocationId, oneshot::Sender<ResponseEnvelope>>>>;

/// Encode a serializable value as MessagePack `Bytes`.
fn encode_bytes<T: Serialize>(value: &T) -> Result<Bytes, String> {
    saikuro_core::msgpack::to_vec(value)
        .map(Bytes::from)
        .map_err(|e| e.to_string())
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
    pub peer_capabilities: CapabilitySet,
    pub max_message_size: usize,
    /// Schema registry shared with the runtime; used to merge announced schemas.
    pub schema_registry: SchemaRegistry,
    /// Provider registry shared with the runtime; used to register/deregister
    /// wire-forwarding provider handles when the peer announces its schema.
    pub provider_registry: ProviderRegistry,
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
    #[instrument(skip(self), fields(peer = %self.peer_id))]
    pub async fn run(mut self) {
        info!(peer = %self.peer_id, "connection established");

        // Shared pending-call map: ForwardTask writes response_tx into this;
        // the recv loop reads it when a ResponseEnvelope arrives from the peer.
        let pending: PendingCalls = Arc::new(Mutex::new(BTreeMap::new()));

        // Channel through which the ForwardTask sends frames TO the peer.
        // The recv loop serialises all outbound writes through `self.sender`.
        let (forward_tx, mut forward_rx) =
            mpsc::channel::<Bytes>(saikuro_exec::ChannelCapacity::MAX);

        loop {
            saikuro_exec::select! {
                // Outbound frame from the ForwardTask (call forwarded to this
                // peer acting as a provider).
                frame_opt = forward_rx.recv() => {
                    match frame_opt {
                        Some(frame) => {
                            if let Err(e) = self.sender.send(frame).await {
                                error!(peer = %self.peer_id, "send error on forwarded call: {e}");
                                break;
                            }
                        }
                        None => {
                            info!(peer = %self.peer_id, "forward channel closed");
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
                            info!(peer = %self.peer_id, "connection closed by peer");
                            break;
                        }
                        Err(e) => {
                            error!(peer = %self.peer_id, "recv error: {e}");
                            break;
                        }
                    }
                }
            }
        }

        // Clean up: deregister any provider the peer announced.
        self.provider_registry
            .deregister(&self.peer_id, self.registration_token);
        self.schema_registry
            .deregister_provider(&self.peer_id, self.registration_token);

        info!(peer = %self.peer_id, "connection handler exiting");
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
    ) -> Option<(ResponseEnvelope, Option<Schema>)> {
        // 1. Decode the MessagePack envelope.
        let envelope = match self.decode_envelope(&frame) {
            Ok(e) => e,
            Err(Some(resp)) => return Some((*resp, None)),
            Err(None) => return None,
        };

        let id = envelope.id;
        debug!(peer = %self.peer_id, %id, target = %envelope.target, "received envelope");

        // 2. Handle system envelopes before schema validation.
        match envelope.invocation_type {
            InvocationType::Announce => {
                let response = self.handle_announce(envelope, pending, forward_tx);
                // If sandbox mode is on and the announce succeeded, build the
                // filtered schema to push back to the peer.
                let sandbox_schema = if self.capability_engine.is_sandboxed() && response.ok {
                    self.build_filtered_schema()
                } else {
                    None
                };
                return Some((response, sandbox_schema));
            }
            InvocationType::Log => {
                // Let the router's log sink handle it:  no validation needed.
                return Some((self.router.dispatch(envelope).await, None));
            }
            _ => {}
        }

        // 3. Validate the envelope against the schema.
        let validation = match self.validator.validate(&envelope) {
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

        // 5. Route to provider.
        Some((self.router.dispatch(envelope).await, None))
    }

    /// Decode a MessagePack frame into an [`Envelope`], or return an error
    /// response on failure.
    fn decode_envelope(&self, frame: &[u8]) -> Result<Envelope, Option<Box<ResponseEnvelope>>> {
        match saikuro_core::msgpack::from_slice(frame) {
            Ok(env) => Ok(env),
            Err(e) => {
                warn!(peer = %self.peer_id, "envelope decode failed: {e}");
                let id = match InvocationId::new() {
                    Ok(id) => id,
                    Err(error) => {
                        error!(peer = %self.peer_id, %error, "cannot generate malformed-envelope response ID");
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
                Err(error) => {
                    error!(peer = %self.peer_id, %error, "cannot generate oversized-frame response ID");
                    return false;
                }
            };
            let response = ResponseEnvelope::err(id, err);
            let _ = self.send_response(response).await;
            return true;
        }

        // Try to decode as ResponseEnvelope first.
        if let Ok(resp) = saikuro_core::msgpack::from_slice::<ResponseEnvelope>(&frame) {
            if let Some(sender) = pending.lock().remove(&resp.id) {
                let _ = sender.send(resp);
                return true;
            }
        }

        let Some((response, sandbox_schema)) = self.handle_frame(frame, pending, forward_tx).await
        else {
            return false;
        };

        if let Err(e) = self.send_response(response).await {
            error!(peer = %self.peer_id, "send error: {e}");
            return false;
        }

        if let Some(filtered) = sandbox_schema {
            if let Err(e) = self.push_sandbox_schema(filtered).await {
                error!(peer = %self.peer_id, "failed to push sandbox schema: {e}");
                return false;
            }
        }

        true
    }

    /// Handle a schema-announcement envelope.
    fn handle_announce(
        &self,
        envelope: Envelope,
        pending: &PendingCalls,
        forward_tx: &mpsc::Sender<Bytes>,
    ) -> ResponseEnvelope {
        let id = envelope.id;

        let schema: Option<Schema> = envelope.args.into_iter().next().and_then(|v| {
            let bytes = encode_bytes(&v).ok()?;
            saikuro_core::msgpack::from_slice(&bytes).ok()
        });

        match schema {
            Some(s) => {
                let ns_count = s.namespaces.len();
                let namespaces: Vec<String> = s.namespaces.keys().cloned().collect();

                match self.schema_registry.merge_schema_with_token(
                    s,
                    &self.peer_id,
                    self.registration_token,
                ) {
                    Ok(()) => {
                        info!(
                            peer = %self.peer_id,
                            namespaces = ns_count,
                            "schema announced and merged"
                        );

                        // Register a wire-forwarding provider handle so the
                        // router can dispatch calls to this peer.
                        self.register_wire_provider(namespaces, pending, forward_tx);

                        ResponseEnvelope::ok_empty(id)
                    }
                    Err(e) => {
                        warn!(peer = %self.peer_id, "schema merge failed: {e}");
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
                warn!(peer = %self.peer_id, "announce envelope has no valid Schema in args[0]");
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
    fn register_wire_provider(
        &self,
        namespaces: Vec<String>,
        pending: &PendingCalls,
        forward_tx: &mpsc::Sender<Bytes>,
    ) {
        let (work_tx, mut work_rx) =
            mpsc::channel::<ProviderWorkItem>(saikuro_exec::ChannelCapacity::MAX);
        let handle = ProviderHandle::with_registration_token(
            self.peer_id.clone(),
            self.registration_token,
            namespaces,
            work_tx,
        );
        self.provider_registry.register(handle);

        let pending_clone = pending.clone();
        let forward_tx_clone = forward_tx.clone();
        let peer_id = self.peer_id.clone();

        spawn(async move {
            while let Some(item) = work_rx.recv().await {
                let frame = match encode_bytes(&item.envelope) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        warn!(peer = %peer_id, "failed to encode forwarded call: {e}");
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
                };

                // Register before sending so an incoming response can find the
                // pending entry; remove it if the send fails to preserve the
                // original orphan-prevention behavior.
                if let Some(resp_tx) = item.response_tx {
                    pending_clone.lock().insert(item.envelope.id, resp_tx);
                }

                // Send the frame to the peer (via the connection handler's sender).
                if forward_tx_clone.send(frame).await.is_err() {
                    warn!(peer = %peer_id, "forward channel closed; provider disconnected");
                    pending_clone.lock().remove(&item.envelope.id);
                    break;
                }
            }
            debug!(peer = %peer_id, "wire-forward task exiting");
        });
    }

    /// Build a capability-filtered schema snapshot for a sandboxed peer.
    ///
    /// Only namespaces and functions visible to `peer_capabilities` (and not
    /// `Internal` or `Private`) are included.
    fn build_filtered_schema(&self) -> Option<Schema> {
        let full = match self.schema_registry.snapshot() {
            Ok(schema) => schema,
            Err(e) => {
                error!(peer = %self.peer_id, error = %e, "schema snapshot capacity exceeded");
                return None;
            }
        };
        let mut filtered = Schema::new();
        // Copy types:  they are passive descriptors and always included.
        filtered.types = full.types.clone();

        for (ns_name, ns_schema) in full.namespaces.iter() {
            let accessible = self.capability_engine.filter_accessible_functions(
                ns_schema.functions.iter().map(|(n, s)| (n.as_str(), s)),
                &self.peer_capabilities,
            );
            if accessible.is_empty() {
                continue;
            }
            let functions = Box::new(
                ns_schema
                    .functions
                    .iter()
                    .filter(|(name, _)| accessible.contains(name))
                    .map(|(name, schema)| (name.clone(), schema.clone()))
                    .collect(),
            );
            filtered
                .namespaces
                .insert(
                    ns_name.clone(),
                    saikuro_core::schema::NamespaceSchema {
                        functions,
                        doc: ns_schema.doc.clone(),
                    },
                )
                .ok();
        }

        Some(filtered)
    }

    /// Encode `filtered_schema` as a `Value` and push it as an unsolicited
    /// `Announce` frame to the peer.  The peer uses this to discover what it
    /// is allowed to call.
    async fn push_sandbox_schema(&mut self, filtered: Schema) -> Result<(), String> {
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
        info!(peer = %self.peer_id, "pushing sandbox-filtered schema to peer");
        self.sender.send(frame).await.map_err(|e| e.to_string())
    }

    async fn send_response(&mut self, response: ResponseEnvelope) -> Result<(), String> {
        let frame = encode_bytes(&response).map_err(|e| format!("response encode error: {e}"))?;
        self.sender.send(frame).await.map_err(|e| e.to_string())
    }
}
