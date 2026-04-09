//! Saikuro async client.
//!
//! Multiplexes call/cast/stream/channel/resource/log/batch over one transport connection using
//! invocation IDs as correlation keys.
//!

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use bytes::Bytes;
use dashmap::DashMap;
use saikuro_core::{
    envelope::{Envelope, InvocationType, ResponseEnvelope, StreamControl},
    error::{ErrorCode, ErrorDetail},
    invocation::InvocationId,
    value::Value as CoreValue,
    PROTOCOL_VERSION,
};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, warn};

use crate::{
    error::{Error, Result},
    transport::{connect, AdapterTransport},
    value::{core_to_json, json_to_core},
    Value,
};

/// Options for [`Client`].
#[derive(Debug, Clone, Default)]
pub struct ClientOptions {
    /// Default timeout for `call` invocations. `None` means no timeout.
    pub default_timeout: Option<Duration>,
}

// An async stream of values received from a provider.

type StreamItem = Result<Value>;

/// An async stream of values yielded by a provider-side stream function.
///
/// Obtained from [`Client::stream`].
pub struct SaikuroStream {
    receiver: mpsc::Receiver<StreamItem>,
}

impl SaikuroStream {
    fn new(receiver: mpsc::Receiver<StreamItem>) -> Self {
        Self { receiver }
    }

    /// Receive the next item from the stream.
    ///
    /// Returns `None` when the stream is closed.
    pub async fn next(&mut self) -> Option<StreamItem> {
        self.receiver.recv().await
    }
}

/// A bidirectional channel opened with [`Client::channel`].
///
/// Use [`SaikuroChannel::send`] to push items to the provider and
/// [`SaikuroChannel::next`] to receive items from the provider.
pub struct SaikuroChannel {
    id: InvocationId,
    send_tx: mpsc::Sender<Bytes>,
    receiver: mpsc::Receiver<StreamItem>,
}

impl SaikuroChannel {
    fn new(
        id: InvocationId,
        send_tx: mpsc::Sender<Bytes>,
        receiver: mpsc::Receiver<StreamItem>,
    ) -> Self {
        Self {
            id,
            send_tx,
            receiver,
        }
    }

    /// Close the channel by sending a StreamControl::End frame.
    pub async fn close(&self) -> Result<()> {
        let mut envelope =
            make_envelope_with_id(self.id, InvocationType::Channel, "", vec![], None);
        envelope.stream_control = Some(StreamControl::End);
        let bytes = envelope
            .to_msgpack()
            .map_err(|e| Error::Codec(e.to_string()))?;
        self.send_tx
            .send(Bytes::from(bytes))
            .await
            .map_err(|_| Error::Transport("client send channel closed".into()))
    }

    /// Abort the channel by sending a StreamControl::Abort frame.
    pub async fn abort(&self) -> Result<()> {
        let mut envelope =
            make_envelope_with_id(self.id, InvocationType::Channel, "", vec![], None);
        envelope.stream_control = Some(StreamControl::Abort);
        let bytes = envelope
            .to_msgpack()
            .map_err(|e| Error::Codec(e.to_string()))?;
        self.send_tx
            .send(Bytes::from(bytes))
            .await
            .map_err(|_| Error::Transport("client send channel closed".into()))
    }

    /// Send a value to the provider side of this channel.
    pub async fn send(&self, value: Value) -> Result<()> {
        let envelope =
            make_envelope_with_id(self.id, InvocationType::Channel, "", vec![value], None);
        let bytes = envelope
            .to_msgpack()
            .map_err(|e| Error::Codec(e.to_string()))?;
        self.send_tx
            .send(Bytes::from(bytes))
            .await
            .map_err(|_| Error::Transport("client send channel closed".into()))
    }

    /// Receive the next inbound channel item.
    ///
    /// Returns `None` when the channel is closed.
    pub async fn next(&mut self) -> Option<StreamItem> {
        self.receiver.recv().await
    }
}

// ---------------------------------------------------------------------------
// Internal routing
// ---------------------------------------------------------------------------

enum PendingSlot {
    /// A one-shot call waiting for a single response.
    Call(oneshot::Sender<ResponseEnvelope>),
    /// An open stream accumulating items.
    Stream(mpsc::Sender<StreamItem>),
    /// An open bidirectional channel accumulating inbound items.
    Channel(mpsc::Sender<StreamItem>),
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Async Saikuro client over a single transport connection.
///
/// The client spawns a background I/O task that drives outbound sends and
/// routes inbound responses back to their waiting callers via in-process
/// channels.  All public methods are `&self` and can be called concurrently
/// from multiple tasks.
pub struct Client {
    /// Send half of the outbound frame channel.
    send_tx: mpsc::Sender<Bytes>,
    /// Pending calls and open streams, keyed by invocation ID.
    pending: Arc<DashMap<InvocationId, PendingSlot>>,
    /// Background I/O task handle.
    recv_task: Option<tokio::task::JoinHandle<()>>,
    /// Whether the client is still connected.
    connected: Arc<AtomicBool>,
    options: ClientOptions,
}

impl Client {
    /// Connect to a Saikuro runtime at `address` and return a ready client.
    pub async fn connect(address: impl AsRef<str>) -> Result<Self> {
        let address = address.as_ref();
        debug!(address = %address, "client connecting");
        let transport = connect(address).await?;
        Self::from_transport(transport, None)
    }

    /// Connect with custom options.
    pub async fn connect_with_options(
        address: impl AsRef<str>,
        options: ClientOptions,
    ) -> Result<Self> {
        let address = address.as_ref();
        let transport = connect(address).await?;
        Self::from_transport(transport, Some(options))
    }

    /// Construct a client from an already-connected transport.
    ///
    /// Starts the background I/O task immediately.  The task first drains any
    /// announce frames already waiting in the transport (which happens when a
    /// provider and client share an in-process transport pair directly), then
    /// enters the normal send/receive loop.
    pub fn from_transport(
        mut transport: Box<dyn AdapterTransport>,
        options: Option<ClientOptions>,
    ) -> Result<Self> {
        let options = options.unwrap_or_default();
        let pending: Arc<DashMap<InvocationId, PendingSlot>> = Arc::new(DashMap::new());
        let connected = Arc::new(AtomicBool::new(true));

        // Outbound frame channel: callers push frames here; the I/O task
        // drains them and writes to the transport.  The channel capacity is
        // large enough that a burst of concurrent calls never blocks a caller.
        let (send_tx, mut send_rx) = mpsc::channel::<Bytes>(256);

        let pending_recv = pending.clone();
        let connected_recv = connected.clone();

        let recv_task = tokio::spawn(async move {
            // ----------------------------------------------------------------
            // Handshake phase: drain any announce frames that may have arrived
            // before this task started.  This is the normal path when a
            // provider and client are connected directly via InMemoryTransport
            // (e.g. integration tests), where the provider sends its announce
            // before the client task is even spawned.
            //
            // We use try_recv rather than a timeout-based poll so that the
            // phase is instant for normal runtime connections (where no announce
            // arrives on the client side at all).
            drain_announces(&mut *transport).await;

            // ----------------------------------------------------------------
            // I/O loop: multiplex outbound sends and inbound responses.
            loop {
                tokio::select! {
                    // Forward outbound frames from callers to the transport.
                    frame = send_rx.recv() => {
                        match frame {
                            Some(f) => {
                                if let Err(e) = transport.send(f).await {
                                    error!(error = %e, "client send error");
                                    break;
                                }
                            }
                            None => break, // all Client handles dropped
                        }
                    }

                    // Route inbound response frames to their waiting callers.
                    result = transport.recv() => {
                        match result {
                            Ok(Some(frame)) => {
                                handle_inbound(frame, &mut *transport, &pending_recv).await;
                            }
                            Ok(None) => {
                                debug!("client: transport closed");
                                break;
                            }
                            Err(e) => {
                                error!(error = %e, "client recv error");
                                break;
                            }
                        }
                    }
                }
            }

            connected_recv.store(false, Ordering::SeqCst);
            teardown_pending(&pending_recv);
            let _ = transport.close().await;
        });

        Ok(Self {
            send_tx,
            pending,
            recv_task: Some(recv_task),
            connected,
            options,
        })
    }

    /// `true` if the client is currently connected.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Gracefully close the client.
    pub async fn close(mut self) -> Result<()> {
        drop(self.send_tx);
        if let Some(task) = self.recv_task.take() {
            let _ = tokio::time::timeout(Duration::from_secs(5), task).await;
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Invocation API
    // -----------------------------------------------------------------------

    /// Perform a request/response call and return the result.
    pub async fn call(&self, target: impl Into<String>, args: Vec<Value>) -> Result<Value> {
        self.call_with_timeout(target, args, self.options.default_timeout)
            .await
    }

    /// Perform a call with an explicit timeout override.
    pub async fn call_with_timeout(
        &self,
        target: impl Into<String>,
        args: Vec<Value>,
        timeout: Option<Duration>,
    ) -> Result<Value> {
        let target = target.into();
        let envelope = make_envelope(InvocationType::Call, &target, args, None);
        let id = envelope.id;

        let (tx, rx) = oneshot::channel();
        self.pending.insert(id, PendingSlot::Call(tx));

        self.send_envelope(&envelope).await?;

        let recv_fut = async {
            rx.await
                .map_err(|_| Error::Transport("pending call dropped".into()))
        };

        let resp = match timeout {
            Some(t) => tokio::time::timeout(t, recv_fut)
                .await
                .map_err(|_| Error::Timeout {
                    target: target.clone(),
                    ms: t.as_millis() as u64,
                })?,
            None => recv_fut.await,
        }?;

        response_to_result(resp)
    }

    /// Fire-and-forget invocation. No response is expected.
    pub async fn cast(&self, target: impl Into<String>, args: Vec<Value>) -> Result<()> {
        let target = target.into();
        let envelope = make_envelope(InvocationType::Cast, &target, args, None);
        self.send_envelope(&envelope).await
    }

    /// Open a server-to-client stream.
    ///
    /// Returns a [`SaikuroStream`] that yields items as they arrive.
    pub async fn stream(
        &self,
        target: impl Into<String>,
        args: Vec<Value>,
    ) -> Result<SaikuroStream> {
        let target = target.into();
        let envelope = make_envelope(InvocationType::Stream, &target, args, None);
        let id = envelope.id;

        let (tx, rx) = mpsc::channel(128);
        self.pending.insert(id, PendingSlot::Stream(tx));

        self.send_envelope(&envelope).await?;
        Ok(SaikuroStream::new(rx))
    }

    /// Execute multiple calls in a single batch envelope and return all results.
    ///
    /// Results are returned in the same order as `calls`.  Individual item
    /// failures are represented as `null` in the result array (matching the
    /// provider-side batch semantics).
    pub async fn batch(&self, calls: Vec<(String, Vec<Value>)>) -> Result<Vec<Value>> {
        let batch_items: Vec<Envelope> = calls
            .into_iter()
            .map(|(target, args)| make_envelope(InvocationType::Call, &target, args, None))
            .collect();

        let batch_env = Envelope {
            version: PROTOCOL_VERSION,
            invocation_type: InvocationType::Batch,
            id: InvocationId::new(),
            target: "$batch".into(),
            args: vec![],
            meta: Default::default(),
            capability: None,
            batch_items: Some(batch_items),
            stream_control: None,
            seq: None,
        };
        let id = batch_env.id;

        let (tx, rx) = oneshot::channel();
        self.pending.insert(id, PendingSlot::Call(tx));
        self.send_envelope(&batch_env).await?;

        let resp = rx
            .await
            .map_err(|_| Error::Transport("batch pending call dropped".into()))?;

        let overall = response_to_result(resp)?;

        // The provider returns the batch result as a JSON array.
        match overall {
            Value::Array(items) => Ok(items),
            other => Ok(vec![other]),
        }
    }

    /// Open a bidirectional channel.
    ///
    /// Use the returned [`SaikuroChannel`] to send and receive values.
    pub async fn channel(
        &self,
        target: impl Into<String>,
        args: Vec<Value>,
    ) -> Result<SaikuroChannel> {
        let target = target.into();
        let envelope = make_envelope(InvocationType::Channel, &target, args, None);
        let id = envelope.id;

        let (tx, rx) = mpsc::channel(128);
        self.pending.insert(id, PendingSlot::Channel(tx));

        self.send_envelope(&envelope).await?;
        Ok(SaikuroChannel::new(id, self.send_tx.clone(), rx))
    }

    /// Invoke a resource-producing function and return the resource payload.
    pub async fn resource(&self, target: impl Into<String>, args: Vec<Value>) -> Result<Value> {
        let target = target.into();
        let envelope = make_envelope(InvocationType::Resource, &target, args, None);
        let id = envelope.id;

        let (tx, rx) = oneshot::channel();
        self.pending.insert(id, PendingSlot::Call(tx));

        self.send_envelope(&envelope).await?;

        let resp = rx
            .await
            .map_err(|_| Error::Transport("pending resource call dropped".into()))?;

        response_to_result(resp)
    }

    /// Forward a structured log record to the runtime log sink.
    ///
    /// This is fire-and-forget and does not wait for a response.
    pub async fn log(
        &self,
        level: impl Into<String>,
        name: impl Into<String>,
        msg: impl Into<String>,
        fields: Option<Value>,
    ) -> Result<()> {
        let mut record = serde_json::Map::new();
        record.insert("level".to_owned(), Value::String(level.into()));
        record.insert("name".to_owned(), Value::String(name.into()));
        record.insert("msg".to_owned(), Value::String(msg.into()));
        if let Some(extra) = fields {
            record.insert("fields".to_owned(), extra);
        }

        let envelope = make_envelope(
            InvocationType::Log,
            "$log",
            vec![Value::Object(record)],
            None,
        );
        self.send_envelope(&envelope).await
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    async fn send_envelope(&self, envelope: &Envelope) -> Result<()> {
        let bytes = envelope
            .to_msgpack()
            .map_err(|e| Error::Codec(e.to_string()))?;
        self.send_tx
            .send(Bytes::from(bytes))
            .await
            .map_err(|_| Error::Transport("client send channel closed".into()))
    }
}

// ---------------------------------------------------------------------------
// Background task helpers
// ---------------------------------------------------------------------------

/// Drain any announce envelopes that have already arrived on the transport.
///
/// This is called once at the start of the I/O task, before the main select
/// loop.  In a real runtime deployment, announce frames are sent by the
/// *provider* to the runtime, never to the client; this path is only active
/// when a provider and client are wired together directly via
/// [`InMemoryTransport`](crate::transport::InMemoryTransport) in tests.
///
/// We process announces using a non-blocking `try_recv`-style loop: poll the
/// transport with a short deadline and ack any announce frames, stopping as
/// soon as nothing is immediately available.  This avoids an indefinite wait
/// on a connection where the provider has not yet sent its announce.
async fn drain_announces(transport: &mut dyn AdapterTransport) {
    // Use a very short deadline per poll so that on real runtime connections
    // (where no announce will ever arrive on the client side) we escape
    // immediately after the first timeout.
    const POLL_TIMEOUT: Duration = Duration::from_millis(20);

    while let Ok(Ok(Some(frame))) = tokio::time::timeout(POLL_TIMEOUT, transport.recv()).await {
        // Check if this is an announce.  If it is, ack it and continue
        // draining.  If it is a normal response, we cannot put it back
        // into the transport; log an unexpected-frame warning and drop
        // it.  In practice this should never happen: no pending call
        // exists yet when this runs.
        if let Ok(env) = Envelope::from_msgpack(&frame) {
            if env.invocation_type == InvocationType::Announce {
                let ack = ResponseEnvelope::ok_empty(env.id);
                if let Ok(ack_bytes) = ack.to_msgpack() {
                    let _ = transport.send(Bytes::from(ack_bytes)).await;
                }
                // Continue: there could be more queued frames
                // (unlikely, but be thorough).
                continue;
            }
        }
        // Non-announce frame arrived before any pending slot exists;
        // this is unexpected.
        warn!("client: unexpected frame during handshake phase, discarding");
    }
}

/// Process a single inbound frame from the transport.
///
/// Tries to decode it as a [`ResponseEnvelope`] and route it to its pending
/// slot.  If decoding fails, logs a warning.  Announce frames that slip
/// through after the handshake phase (should not happen in practice) are
/// acked so that a misbehaving peer does not stall.
async fn handle_inbound(
    frame: Bytes,
    transport: &mut dyn AdapterTransport,
    pending: &DashMap<InvocationId, PendingSlot>,
) {
    if let Ok(resp) = ResponseEnvelope::from_msgpack(&frame) {
        route_response(resp, pending).await;
        return;
    }

    // Fall back to Envelope: this catches late/duplicate announces.
    if let Ok(env) = Envelope::from_msgpack(&frame) {
        if env.invocation_type == InvocationType::Announce {
            let ack = ResponseEnvelope::ok_empty(env.id);
            if let Ok(ack_bytes) = ack.to_msgpack() {
                let _ = transport.send(Bytes::from(ack_bytes)).await;
            }
        } else {
            warn!(
                target = %env.target,
                invocation_type = %env.invocation_type,
                "client received unexpected inbound envelope"
            );
        }
        return;
    }

    warn!("client: received undecodable inbound frame");
}

async fn route_response(resp: ResponseEnvelope, pending: &DashMap<InvocationId, PendingSlot>) {
    let id = resp.id;
    let is_stream_end = resp
        .stream_control
        .as_ref()
        .map(|c| matches!(c, StreamControl::End | StreamControl::Abort))
        .unwrap_or(false);
    let is_error = !resp.ok;

    let slot_type = pending.get(&id).map(|s| match s.value() {
        PendingSlot::Call(_) => "call",
        PendingSlot::Stream(_) => "stream",
        PendingSlot::Channel(_) => "channel",
    });

    match slot_type {
        Some("call") => {
            if let Some((_, PendingSlot::Call(tx))) = pending.remove(&id) {
                let _ = tx.send(resp);
            }
        }
        Some("stream") => {
            if let Some(slot) = pending.get(&id) {
                if let PendingSlot::Stream(tx) = slot.value() {
                    if is_stream_end {
                        drop(slot);
                        pending.remove(&id);
                    } else if is_error {
                        let detail = resp.error.unwrap_or_else(|| {
                            ErrorDetail::new(ErrorCode::Internal, "stream error")
                        });
                        let _ = tx.try_send(Err(Error::remote(
                            detail.code.to_string(),
                            detail.message,
                            None,
                        )));
                        drop(slot);
                        pending.remove(&id);
                    } else {
                        let value = resp.result.map(core_to_json).unwrap_or(Value::Null);
                        let _ = tx.try_send(Ok(value));
                    }
                }
            }
        }
        Some("channel") => {
            if let Some(slot) = pending.get(&id) {
                if let PendingSlot::Channel(tx) = slot.value() {
                    let tx = tx.clone();
                    drop(slot); // Drop DashMap guard before await
                    if is_stream_end {
                        // Remove pending slot on stream end
                        pending.remove(&id);
                    } else if is_error {
                        let detail = resp.error.unwrap_or_else(|| {
                            ErrorDetail::new(ErrorCode::Internal, "channel error")
                        });
                        let _ = tx
                            .send(Err(Error::remote(
                                detail.code.to_string(),
                                detail.message,
                                None,
                            )))
                            .await;
                        pending.remove(&id);
                    } else {
                        let value = resp.result.map(core_to_json).unwrap_or(Value::Null);
                        if tx.send(Ok(value)).await.is_err() {
                            pending.remove(&id);
                        }
                        // Keep slot alive for subsequent messages unless send failed
                    }
                }
            }
        }
        _ => {
            debug!(id = %id, "received response for unknown invocation id");
        }
    }
}

fn teardown_pending(pending: &DashMap<InvocationId, PendingSlot>) {
    let keys: Vec<InvocationId> = pending.iter().map(|e| *e.key()).collect();
    for key in keys {
        if let Some((_, slot)) = pending.remove(&key) {
            match slot {
                PendingSlot::Call(tx) => drop(tx),
                PendingSlot::Stream(tx) => {
                    let _ = tx.try_send(Err(Error::Transport("connection lost".into())));
                }
                PendingSlot::Channel(tx) => {
                    let _ = tx.try_send(Err(Error::Transport("connection lost".into())));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_envelope(
    inv_type: InvocationType,
    target: &str,
    args: Vec<Value>,
    capability: Option<saikuro_core::capability::CapabilityToken>,
) -> Envelope {
    make_envelope_with_id(InvocationId::new(), inv_type, target, args, capability)
}

fn make_envelope_with_id(
    id: InvocationId,
    inv_type: InvocationType,
    target: &str,
    args: Vec<Value>,
    capability: Option<saikuro_core::capability::CapabilityToken>,
) -> Envelope {
    let core_args: Vec<CoreValue> = args.into_iter().map(json_to_core).collect();
    Envelope {
        version: PROTOCOL_VERSION,
        invocation_type: inv_type,
        id,
        target: target.to_owned(),
        args: core_args,
        meta: Default::default(),
        capability,
        batch_items: None,
        stream_control: None,
        seq: None,
    }
}

/// Convert a [`ResponseEnvelope`] into a `Result<Value>`.
fn response_to_result(resp: ResponseEnvelope) -> Result<Value> {
    if resp.ok {
        Ok(resp.result.map(core_to_json).unwrap_or(Value::Null))
    } else {
        let detail = resp
            .error
            .unwrap_or_else(|| ErrorDetail::new(ErrorCode::Internal, "call failed with no detail"));
        Err(Error::remote(detail.code.to_string(), detail.message, None))
    }
}
