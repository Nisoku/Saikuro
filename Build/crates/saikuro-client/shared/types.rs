//! Cross-platform type definitions for the client/provider.

use alloc::string::ToString;
use core::time::Duration;

use saikuro_core::envelope::ResponseEnvelope;
use saikuro_core::invocation::InvocationId;
use saikuro_event::{Result, SaikuroError};
use saikuro_exec::{mpsc, oneshot, sync::Mutex};

pub use crate::Value;

// Portable atomics

/// Portable `Arc`: `alloc::sync::Arc` when atomics are available,
/// `portable_atomic_util::Arc` on targets without hardware atomics.
#[cfg(target_has_atomic = "ptr")]
pub type Arc<T> = alloc::sync::Arc<T>;
#[cfg(not(target_has_atomic = "ptr"))]
pub type Arc<T> = portable_atomic_util::Arc<T>;

/// Portable atomic u64: `core::sync::atomic::AtomicU64` when available,
/// `portable_atomic::AtomicU64` otherwise.
#[cfg(target_has_atomic = "64")]
pub type AtomicU64 = core::sync::atomic::AtomicU64;
#[cfg(not(target_has_atomic = "64"))]
pub type AtomicU64 = portable_atomic::AtomicU64;

/// Portable `AtomicBool`.
#[cfg(target_has_atomic = "8")]
pub type AtomicBool = core::sync::atomic::AtomicBool;
#[cfg(not(target_has_atomic = "8"))]
pub type AtomicBool = portable_atomic::AtomicBool;

/// Portable ordering constants.
#[cfg(target_has_atomic = "8")]
pub use core::sync::atomic::Ordering;
#[cfg(not(target_has_atomic = "8"))]
pub use portable_atomic::Ordering;

// Channel capacity

/// Default capacity for the outbound frame channel and stream/channel buffers.
pub(crate) const CHANNEL_CAPACITY: saikuro_exec::ChannelCapacity =
    saikuro_exec::ChannelCapacity::MAX;

/// Capacity for stream and channel pending item buffers.
pub(crate) const STREAM_CHANNEL_CAPACITY: saikuro_exec::ChannelCapacity =
    saikuro_exec::ChannelCapacity::DEFAULT;

// Client types

/// Options for [`Client`](super::client::Client).
#[derive(Debug, Clone, Default)]
pub struct ClientOptions {
    /// Default timeout for `call` invocations. `None` means no timeout.
    pub default_timeout: Option<Duration>,
}

/// An async stream of values received from a provider.
pub struct SaikuroStream {
    receiver: mpsc::Receiver<StreamItem>,
}

impl SaikuroStream {
    pub(crate) fn new(receiver: mpsc::Receiver<StreamItem>) -> Self {
        Self { receiver }
    }

    /// Receive the next item from the stream.
    ///
    /// Returns `None` when the stream is closed.
    pub async fn next(&mut self) -> Option<StreamItem> {
        self.receiver.recv().await
    }
}

/// A bidirectional channel opened with
/// [`Client::channel`](super::client::Client::channel).
pub struct SaikuroChannel {
    id: InvocationId,
    send_tx: ChannelSendTx,
    receiver: mpsc::Receiver<StreamItem>,
    outbound_seq: AtomicU64,
}

impl SaikuroChannel {
    pub(crate) fn new(
        id: InvocationId,
        send_tx: ChannelSendTx,
        receiver: mpsc::Receiver<StreamItem>,
    ) -> Self {
        Self {
            id,
            send_tx,
            receiver,
            outbound_seq: AtomicU64::new(0),
        }
    }

    fn next_seq(&self) -> u64 {
        #[cfg(target_has_atomic = "64")]
        {
            self.outbound_seq.fetch_add(1, Ordering::Relaxed)
        }
        #[cfg(not(target_has_atomic = "64"))]
        {
            self.outbound_seq.fetch_add(1, Ordering::Relaxed)
        }
    }

    async fn send_channel_envelope(
        &self,
        envelope: &saikuro_core::envelope::Envelope,
    ) -> Result<()> {
        let bytes = envelope
            .to_msgpack()
            .map_err(|e| SaikuroError::Serialization(e.to_string()))?;
        let send_tx = self.send_tx.lock().await.clone();
        let send_tx =
            send_tx.ok_or_else(|| SaikuroError::SendFailed("client send channel closed".into()))?;
        send_tx
            .send(bytes::Bytes::from(bytes))
            .await
            .map_err(|_| SaikuroError::SendFailed("client send channel closed".into()))
    }

    /// Close the channel by sending a StreamControl::End frame.
    pub async fn close(&self) -> Result<()> {
        use saikuro_core::envelope::{InvocationType, StreamControl};

        let mut envelope = super::helpers::make_envelope_with_id(
            self.id,
            InvocationType::Channel,
            "",
            alloc::vec![],
            Some(self.next_seq()),
        );
        envelope.stream_control = Some(StreamControl::End);
        self.send_channel_envelope(&envelope).await
    }

    /// Abort the channel by sending a StreamControl::Abort frame.
    pub async fn abort(&self) -> Result<()> {
        use saikuro_core::envelope::{InvocationType, StreamControl};

        let mut envelope = super::helpers::make_envelope_with_id(
            self.id,
            InvocationType::Channel,
            "",
            alloc::vec![],
            Some(self.next_seq()),
        );
        envelope.stream_control = Some(StreamControl::Abort);
        self.send_channel_envelope(&envelope).await
    }

    /// Send a value to the provider side of this channel.
    pub async fn send(&self, value: Value) -> Result<()> {
        use saikuro_core::envelope::InvocationType;

        let envelope = super::helpers::make_envelope_with_id(
            self.id,
            InvocationType::Channel,
            "",
            alloc::vec![value],
            Some(self.next_seq()),
        );
        self.send_channel_envelope(&envelope).await
    }

    /// Receive the next inbound channel item.
    ///
    /// Returns `None` when the channel is closed.
    pub async fn next(&mut self) -> Option<StreamItem> {
        self.receiver.recv().await
    }
}

// Internal routing types

/// A stream item or result.
pub type StreamItem = Result<Value>;

/// A channel outbound sender handle, wrapped for shared ownership.
pub type ChannelSendTx = Arc<Mutex<Option<mpsc::Sender<bytes::Bytes>>>>;

/// A pending invocation slot, awaiting a response.
pub(crate) enum PendingSlot {
    /// A one-shot call waiting for a single response.
    Call(oneshot::Sender<ResponseEnvelope>),
    /// An open stream accumulating items.
    Stream(mpsc::Sender<StreamItem>),
    /// An open bidirectional channel accumulating inbound items.
    Channel(mpsc::Sender<StreamItem>),
}
