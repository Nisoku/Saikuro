//! Connection handler:  one instance per connected adapter peer.
//!
//! Each time an adapter connects over any transport backend a
//! [`ConnectionHandler`] is spawned.  It owns the transport halves and
//! drives the read loop: receive a frame -> decode envelope -> validate ->
//! capability-check -> route -> encode response -> send back.
//!
//! ## Dual-role connections
//!
//! A single transport connection can act in **two roles simultaneously**:
//!
//! - **Client role**: the peer sends `Envelope` frames (requests to the
//!   runtime).  The runtime validates, capability-checks, routes, and replies
//!   with a `ResponseEnvelope`.
//!
//! - **Provider role**: after sending an `Announce` envelope, the peer becomes
//!   a provider for its declared namespaces.  When the runtime needs to call
//!   one of those functions, it forwards the `Envelope` to the peer over the
//!   wire and waits for a `ResponseEnvelope` reply.
//!
//! The handler distinguishes the two frame types by the presence of the `ok`
//! field: `ResponseEnvelope` always serialises an `ok` boolean; `Envelope`
//! serialises a `type` field instead.  We use a peek-decode strategy to
//! classify incoming frames.
//!
//! ## System envelopes
//!
//! - `Announce`: merges the declared [`Schema`] into the live registry AND
//!   registers a wire-forwarding [`ProviderHandle`] so that subsequent calls
//!   from any client are forwarded to this peer.  In **sandbox mode**, after
//!   the ok response, a second unsolicited `Announce` frame carrying the
//!   capability-filtered schema snapshot is pushed back to the peer.
//!
//! - `Log`: forwarded directly to the router's log sink.
//!
//! Connections are fully independent; a crash in one handler does not
//! affect others.

use bytes::Bytes;
use dashmap::DashMap;
use saikuro_core::{
    capability::CapabilitySet,
    envelope::{Envelope, InvocationType},
    error::ErrorDetail,
    invocation::InvocationId,
    schema::Schema,
    value::Value,
    ResponseEnvelope,
};
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
use futures::future::FutureExt;
use saikuro_transport::traits::{TransportReceiver, TransportSender};
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};

//  Pending call map

/// Tracks in-flight `Call` invocations forwarded to a wire-connected provider.
/// Maps `InvocationId -> oneshot::Sender<ResponseEnvelope>`.
type PendingCalls = Arc<DashMap<InvocationId, oneshot::Sender<ResponseEnvelope>>>;

/// A handler for a single connected peer.
///
/// Generic over the transport halves so it works with every backend and
/// compiles cleanly on wasm32 targets.
pub struct ConnectionHandler<S, R>
where
    S: TransportSender,
    R: TransportReceiver,
{
    pub peer_id: String,
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
    S: TransportSender,
    R: TransportReceiver,
{
    /// Build a handler in **sandbox mode**.
    ///
    /// In sandbox mode every [`Announce`](InvocationType::Announce) processed by
    /// this handler causes a capability-filtered schema snapshot to be pushed
    /// back to the peer immediately after the `ok` response.  This lets the peer
    /// discover exactly which functions it is allowed to call without trial and
    /// error.
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
    S: TransportSender,
    R: TransportReceiver,
{
    /// Run the receive loop until the connection is closed or an unrecoverable
    /// error occurs.
    ///
    /// The loop classifies each incoming frame:
    /// - If the frame decodes as a `ResponseEnvelope` (has an `ok` field) AND
    ///   matches a pending forwarded call, the response is delivered to the
    ///   caller's oneshot receiver.
    /// - Otherwise the frame is treated as a new `Envelope` from the peer and
    ///   goes through the normal validate -> route -> reply pipeline.
    #[instrument(skip(self), fields(peer = %self.peer_id))]
    pub async fn run(mut self) {
        info!(peer = %self.peer_id, "connection established");

        // Shared pending-call map: ForwardTask writes response_tx into this;
        // the recv loop reads it when a ResponseEnvelope arrives from the peer.
        let pending: PendingCalls = Arc::new(DashMap::new());

        // Channel through which the ForwardTask sends frames TO the peer.
        // The recv loop serialises all outbound writes through `self.sender`.
        let (forward_tx, mut forward_rx) = mpsc::channel::<Bytes>(256);

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
                            if frame.len() > self.max_message_size {
                                let err = saikuro_core::error::ErrorDetail::new(
                                    saikuro_core::error::ErrorCode::MessageTooLarge,
                                    format!(
                                        "frame {} bytes exceeds limit {} bytes",
                                        frame.len(),
                                        self.max_message_size
                                    ),
                                );
                                let response = ResponseEnvelope::err(InvocationId::new(), err);
                                let _ = self.send_response(response).await;
                                continue;
                            }

                            // Classify the frame
                            // Try to decode as ResponseEnvelope first.
                            // ResponseEnvelope has `ok`, `id`, and optionally
                            // `result`/`error`/`seq`/`stream_control`.
                            // Envelope has `type` (the discriminant) as a
                            // required field.  We can tell them apart by
                            // attempting ResponseEnvelope decode and checking if
                            // the resulting `id` matches any pending call.
                            if let Ok(resp) = rmp_serde::from_slice::<ResponseEnvelope>(&frame) {
                                if let Some((_, sender)) = pending.remove(&resp.id) {
                                    // This is a response to a call we forwarded.
                                    let _ = sender.send(resp);
                                    continue;
                                }
                                // The ID is not pending:  fall through and treat
                                // as a normal inbound envelope (will likely fail
                                // validation since it has no `type` field, but
                                // we surface the error cleanly).
                            }

                            let (response, sandbox_schema) =
                                self.handle_frame(frame, &pending, &forward_tx).await;

                            if let Err(e) = self.send_response(response).await {
                                error!(peer = %self.peer_id, "send error: {e}");
                                break;
                            }

                            // In sandbox mode, push the filtered schema
                            // snapshot after each successful Announce.
                            if let Some(filtered) = sandbox_schema {
                                if let Err(e) = self.push_sandbox_schema(filtered).await {
                                    error!(
                                        peer = %self.peer_id,
                                        "failed to push sandbox schema: {e}"
                                    );
                                    break;
                                }
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
        self.provider_registry.deregister(&self.peer_id);
        self.schema_registry.deregister_provider(&self.peer_id);

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
    ) -> (ResponseEnvelope, Option<Schema>) {
        // 1. Decode the MessagePack envelope.
        let envelope: Envelope = match rmp_serde::from_slice(&frame) {
            Ok(env) => env,
            Err(e) => {
                warn!(peer = %self.peer_id, "envelope decode failed: {e}");
                return (
                    ResponseEnvelope::err(
                        InvocationId::new(),
                        ErrorDetail::new(
                            saikuro_core::error::ErrorCode::MalformedEnvelope,
                            format!("msgpack decode error: {e}"),
                        ),
                    ),
                    None,
                );
            }
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
                    Some(self.build_filtered_schema())
                } else {
                    None
                };
                return (response, sandbox_schema);
            }
            InvocationType::Log => {
                // Let the router's log sink handle it:  no validation needed.
                return (self.router.dispatch(envelope).await, None);
            }
            _ => {}
        }

        // 3. Validate the envelope against the schema.
        let validation = match self.validator.validate(&envelope) {
            Ok(report) => report,
            Err(e) => {
                return (
                    ResponseEnvelope::err(id, ErrorDetail::new(e.error_code(), e.to_string())),
                    None,
                );
            }
        };

        // 4. Capability check.
        match self
            .capability_engine
            .check_ref(&self.peer_capabilities, &validation.function_ref)
        {
            CapabilityOutcome::Granted => {}
            CapabilityOutcome::Denied { missing } => {
                return (
                    ResponseEnvelope::err(
                        id,
                        ErrorDetail::new(
                            saikuro_core::error::ErrorCode::CapabilityDenied,
                            format!("caller lacks '{}' to invoke '{}'", missing, envelope.target),
                        ),
                    ),
                    None,
                );
            }
        }

        // 5. Route to provider.
        (self.router.dispatch(envelope).await, None)
    }

    /// Handle a schema-announcement envelope (§6.1 development mode).
    ///
    /// Deserialises the [`Schema`] from `args[0]`, merges it into the live
    /// schema registry, **and** registers a wire-forwarding [`ProviderHandle`]
    /// for each declared namespace so that the runtime can route calls to this
    /// peer.  Returns `ok_empty` on success, an error response on any failure.
    fn handle_announce(
        &self,
        envelope: Envelope,
        pending: &PendingCalls,
        forward_tx: &mpsc::Sender<Bytes>,
    ) -> ResponseEnvelope {
        let id = envelope.id;

        // args[0] must be the serialised Schema (a map value).
        let schema: Option<Schema> = envelope.args.into_iter().next().and_then(|v| {
            let bytes = rmp_serde::to_vec_named(&v).ok()?;
            rmp_serde::from_slice::<Schema>(&bytes).ok()
        });

        match schema {
            Some(s) => {
                let ns_count = s.namespaces.len();
                let namespaces: Vec<String> = s.namespaces.keys().cloned().collect();

                match self.schema_registry.merge_schema(s, &self.peer_id) {
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
                                saikuro_core::error::ErrorCode::Internal,
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
                        saikuro_core::error::ErrorCode::MalformedEnvelope,
                        "announce envelope must carry a Schema in args[0]".to_owned(),
                    ),
                )
            }
        }
    }

    /// Create and register a [`ProviderHandle`] that forwards invocations to
    /// the connected peer over the wire.
    ///
    /// Work items arrive via `work_rx`; the forwarder task encodes the
    /// `Envelope` as a MessagePack frame, sends it to the peer, and records the
    /// `response_tx` oneshot in `pending` so the recv loop can deliver the
    /// reply when it arrives.
    fn register_wire_provider(
        &self,
        namespaces: Vec<String>,
        pending: &PendingCalls,
        forward_tx: &mpsc::Sender<Bytes>,
    ) {
        let (work_tx, mut work_rx) = mpsc::channel::<ProviderWorkItem>(256);
        let handle = ProviderHandle::new(self.peer_id.clone(), namespaces, work_tx);
        self.provider_registry.register(handle);

        let pending_clone = pending.clone();
        let forward_tx_clone = forward_tx.clone();
        let peer_id = self.peer_id.clone();

        spawn(async move {
            while let Some(item) = work_rx.recv().await {
                // Encode the envelope for the wire.
                let frame = match rmp_serde::to_vec_named(&item.envelope) {
                    Ok(bytes) => Bytes::from(bytes),
                    Err(e) => {
                        warn!(peer = %peer_id, "failed to encode forwarded call: {e}");
                        if let Some(tx) = item.response_tx {
                            let _ = tx.send(ResponseEnvelope::err(
                                item.envelope.id,
                                ErrorDetail::new(
                                    saikuro_core::error::ErrorCode::Internal,
                                    format!("encode error: {e}"),
                                ),
                            ));
                        }
                        continue;
                    }
                };

                // If this is a Call, register the response_tx in the pending map.
                if let Some(resp_tx) = item.response_tx {
                    pending_clone.insert(item.envelope.id, resp_tx);
                }

                // Send the frame to the peer (via the connection handler's sender).
                if forward_tx_clone.send(frame).await.is_err() {
                    warn!(peer = %peer_id, "forward channel closed; provider disconnected");
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
    fn build_filtered_schema(&self) -> Schema {
        let full = self.schema_registry.snapshot();
        let mut filtered = Schema::new();
        // Copy types:  they are passive descriptors and always included.
        filtered.types = full.types.clone();

        for (ns_name, ns_schema) in &full.namespaces {
            let accessible = self.capability_engine.filter_accessible_functions(
                ns_schema.functions.iter().map(|(n, s)| (n.as_str(), s)),
                &self.peer_capabilities,
            );
            if accessible.is_empty() {
                continue;
            }
            let functions = ns_schema
                .functions
                .iter()
                .filter(|(name, _)| accessible.contains(name))
                .map(|(name, schema)| (name.clone(), schema.clone()))
                .collect();
            filtered.namespaces.insert(
                ns_name.clone(),
                saikuro_core::schema::NamespaceSchema {
                    functions,
                    doc: ns_schema.doc.clone(),
                },
            );
        }

        filtered
    }

    /// Encode `filtered_schema` as a `Value` and push it as an unsolicited
    /// `Announce` frame to the peer.  The peer uses this to discover what it
    /// is allowed to call.
    async fn push_sandbox_schema(&mut self, filtered: Schema) -> Result<(), String> {
        let schema_value: Value = {
            let bytes = rmp_serde::to_vec_named(&filtered)
                .map_err(|e| format!("sandbox schema encode error: {e}"))?;
            rmp_serde::from_slice::<Value>(&bytes)
                .map_err(|e| format!("sandbox schema value decode error: {e}"))?
        };
        let announce = Envelope::announce(schema_value);
        let frame = rmp_serde::to_vec_named(&announce)
            .map_err(|e| format!("announce frame encode error: {e}"))?;
        info!(peer = %self.peer_id, "pushing sandbox-filtered schema to peer");
        self.sender
            .send(Bytes::from(frame))
            .await
            .map_err(|e| e.to_string())
    }

    async fn send_response(&mut self, response: ResponseEnvelope) -> Result<(), String> {
        let frame = rmp_serde::to_vec_named(&response)
            .map_err(|e| format!("response encode error: {e}"))?;
        self.sender
            .send(Bytes::from(frame))
            .await
            .map_err(|e| e.to_string())
    }
}
