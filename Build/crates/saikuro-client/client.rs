//! Saikuro async client.
//!
//! Multiplexes call/cast/stream/channel/resource/log/batch over one transport
//! connection using invocation IDs as correlation keys.

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::time::Duration;

use bytes::Bytes;
use futures::future::FutureExt;
use saikuro_core::envelope::{Envelope, InvocationType};
use saikuro_core::invocation::InvocationId;
use saikuro_event::Result;
use saikuro_event::{LogLevel, LogRecord, LogSink, SaikuroError};
use saikuro_exec::{mpsc, oneshot, sync::Mutex};
use saikuro_transport::{connect, AdapterTransport};

use crate::shared::helpers::{
    drain_announces, handle_inbound, make_envelope, response_to_result, teardown_pending,
};
use crate::shared::map::{ChannelSenderMap, PendingMap};
use crate::shared::types::{
    Arc, AtomicBool, ChannelSendTx, ClientOptions, Ordering, PendingSlot, SaikuroChannel,
    SaikuroStream, CHANNEL_CAPACITY, STREAM_CHANNEL_CAPACITY,
};
use crate::Value;

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
    pending: Arc<PendingMap>,
    /// Channel-specific outbound sender handles to invalidate on shutdown.
    channel_senders: Arc<ChannelSenderMap>,
    /// Background I/O task handle.
    recv_task: Option<saikuro_exec::JoinHandle<()>>,
    /// Whether the client is still connected.
    connected: Arc<AtomicBool>,
    options: ClientOptions,
    /// The log sink used by this client.
    pub log: Arc<dyn LogSink>,
}

impl Client {
    /// Connect to a Saikuro runtime at `address` and return a ready client.
    pub async fn connect(address: impl AsRef<str>) -> Result<Self> {
        let address = address.as_ref();
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

    /// Connect with a custom log sink.
    pub async fn connect_with_log(
        address: impl AsRef<str>,
        options: Option<ClientOptions>,
        log: Arc<dyn LogSink>,
    ) -> Result<Self> {
        let address = address.as_ref();
        {
            let mut record =
                LogRecord::now(LogLevel::Debug, "saikuro.rust.client", "client connecting");
            record.set_context("address", address.to_owned());
            log.emit(&record).await;
        }
        let transport = connect(address).await?;
        Self::from_transport_with_log(transport, options, log)
    }

    /// Construct a client from an already-connected transport.
    pub fn from_transport(
        transport: Box<dyn AdapterTransport>,
        options: Option<ClientOptions>,
    ) -> Result<Self> {
        Self::from_transport_with_log(
            transport,
            options,
            Arc::from(Box::new(saikuro_event::NullSink) as Box<dyn LogSink>),
        )
    }

    /// Construct a client from an already-connected transport with a log sink.
    pub fn from_transport_with_log(
        mut transport: Box<dyn AdapterTransport>,
        options: Option<ClientOptions>,
        log: Arc<dyn LogSink>,
    ) -> Result<Self> {
        let options = options.unwrap_or_default();
        let pending = Arc::new(PendingMap::new());
        let channel_senders = Arc::new(ChannelSenderMap::new());
        let connected = Arc::new(AtomicBool::new(true));

        let (send_tx, mut send_rx) = mpsc::channel::<Bytes>(CHANNEL_CAPACITY);

        let pending_recv = pending.clone();
        let channel_senders_recv = channel_senders.clone();
        let connected_recv = connected.clone();
        let log_recv = log.clone();

        let recv_task = saikuro_exec::spawn(async move {
            drain_announces(&mut *transport).await;

            loop {
                saikuro_exec::select! {
                    frame = send_rx.recv() => {
                        match frame {
                            Some(f) => {
                                if let Err(e) = transport.send(f).await {
                                    let mut record = LogRecord::now(LogLevel::Error, "saikuro.rust.client", "client send error");
                                    record.set_context("error", alloc::format!("{e}"));
                                    log_recv.emit(&record).await;
                                    break;
                                }
                            }
                            None => break,
                        }
                    }

                    incoming = transport.recv().fuse() => {
                        match incoming {
                            Ok(Some(frame)) => {
                                handle_inbound(
                                    frame,
                                    &mut *transport,
                                    &pending_recv,
                                    &channel_senders_recv,
                                )
                                .await;
                            }
                            Ok(None) => {
                                let record = LogRecord::now(LogLevel::Debug, "saikuro.rust.client", "client: transport closed");
                                log_recv.emit(&record).await;
                                break;
                            }
                            Err(e) => {
                                let mut record = LogRecord::now(LogLevel::Error, "saikuro.rust.client", "client recv error");
                                record.set_context("error", alloc::format!("{e}"));
                                log_recv.emit(&record).await;
                                break;
                            }
                        }
                    }
                }
            }

            connected_recv.store(false, Ordering::SeqCst);
            teardown_pending(&pending_recv);
            channel_senders_recv.clear();
            let _ = transport.close().await;
        });

        Ok(Self {
            send_tx,
            pending,
            channel_senders,
            recv_task: Some(recv_task),
            connected,
            options,
            log,
        })
    }

    /// `true` if the client is currently connected.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Gracefully close the client.
    pub async fn close(mut self) -> Result<()> {
        let channels: Vec<ChannelSendTx> = self.channel_senders.clone_values();
        for channel_send in channels {
            let _ = channel_send.lock().await.take();
        }
        self.channel_senders.clear();

        drop(self.send_tx);
        if let Some(task) = self.recv_task.take() {
            const CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
            let _ = saikuro_exec::timeout(CLOSE_TIMEOUT, task).await;
        }
        Ok(())
    }

    // Invocation API
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
        let envelope = make_envelope(InvocationType::Call, &target, args)?;
        let id = envelope.id;

        let (tx, rx) = oneshot::channel();
        self.pending.insert(id, PendingSlot::Call(tx));

        if let Err(e) = self.send_envelope(&envelope).await {
            self.pending.remove(&id);
            return Err(e);
        }

        let recv_fut = async {
            rx.await
                .map_err(|_| SaikuroError::SendFailed("pending call dropped".into()))
        };

        let resp = match timeout {
            Some(t) => {
                saikuro_exec::timeout(t, recv_fut)
                    .await
                    .map_err(|_| SaikuroError::Timeout {
                        millis: t.as_millis() as u64,
                    })?
            }
            None => recv_fut.await,
        }?;

        response_to_result(resp)
    }

    /// Fire-and-forget invocation. No response is expected.
    pub async fn cast(&self, target: impl Into<String>, args: Vec<Value>) -> Result<()> {
        let target = target.into();
        let envelope = make_envelope(InvocationType::Cast, &target, args)?;
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
        let envelope = make_envelope(InvocationType::Stream, &target, args)?;
        let id = envelope.id;

        let (tx, rx) = mpsc::channel(STREAM_CHANNEL_CAPACITY);
        self.pending.insert(id, PendingSlot::Stream(tx));

        if let Err(e) = self.send_envelope(&envelope).await {
            self.pending.remove(&id);
            return Err(e);
        }
        Ok(SaikuroStream::new(rx))
    }

    /// Execute multiple calls in a single batch envelope and return all results.
    pub async fn batch(&self, calls: Vec<(String, Vec<Value>)>) -> Result<Vec<Value>> {
        let batch_items: Vec<Envelope> = calls
            .into_iter()
            .map(|(target, args)| make_envelope(InvocationType::Call, &target, args))
            .collect::<Result<_>>()?;

        let batch_env = Envelope {
            version: saikuro_core::PROTOCOL_VERSION,
            invocation_type: InvocationType::Batch,
            id: InvocationId::new()?,
            target: "$batch".into(),
            args: alloc::vec![],
            meta: Default::default(),
            capability: None,
            batch_items: Some(batch_items),
            stream_control: None,
            seq: None,
        };
        let id = batch_env.id;

        let (tx, rx) = oneshot::channel();
        self.pending.insert(id, PendingSlot::Call(tx));
        if let Err(e) = self.send_envelope(&batch_env).await {
            self.pending.remove(&id);
            return Err(e);
        }

        let resp = rx
            .await
            .map_err(|_| SaikuroError::SendFailed("batch pending call dropped".into()))?;

        let overall = response_to_result(resp)?;

        match overall {
            Value::Array(items) => Ok(items),
            other => Ok(alloc::vec![other]),
        }
    }

    /// Open a bidirectional channel.
    pub async fn channel(
        &self,
        target: impl Into<String>,
        args: Vec<Value>,
    ) -> Result<SaikuroChannel> {
        let target = target.into();
        let envelope = make_envelope(InvocationType::Channel, &target, args)?;
        let id = envelope.id;

        let (tx, rx) = mpsc::channel(STREAM_CHANNEL_CAPACITY);
        self.pending.insert(id, PendingSlot::Channel(tx));
        let channel_send = Arc::new(Mutex::new(Some(self.send_tx.clone())));
        self.channel_senders.insert(id, channel_send.clone());

        if let Err(e) = self.send_envelope(&envelope).await {
            self.pending.remove(&id);
            self.channel_senders.remove(&id);
            return Err(e);
        }
        Ok(SaikuroChannel::new(id, channel_send, rx))
    }

    /// Invoke a resource-producing function and return the resource payload.
    pub async fn resource(&self, target: impl Into<String>, args: Vec<Value>) -> Result<Value> {
        let target = target.into();
        let envelope = make_envelope(InvocationType::Resource, &target, args)?;
        let id = envelope.id;

        let (tx, rx) = oneshot::channel();
        self.pending.insert(id, PendingSlot::Call(tx));

        if let Err(e) = self.send_envelope(&envelope).await {
            self.pending.remove(&id);
            return Err(e);
        }

        let resp = rx
            .await
            .map_err(|_| SaikuroError::SendFailed("pending resource call dropped".into()))?;

        response_to_result(resp)
    }

    /// Forward a structured log record to the runtime log sink.
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
            alloc::vec![Value::Object(record)],
        )?;
        self.send_envelope(&envelope).await
    }

    // Internal helpers
    async fn send_envelope(&self, envelope: &Envelope) -> Result<()> {
        let bytes = envelope
            .to_msgpack()
            .map_err(|e| SaikuroError::Serialization(e.to_string()))?;
        self.send_tx
            .send(Bytes::from(bytes))
            .await
            .map_err(|_| SaikuroError::SendFailed("client send channel closed".into()))
    }
}
