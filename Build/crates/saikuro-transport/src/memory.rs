//! In-memory transport.
//!
//! Two tasks in the same process communicate via a pair of bounded MPSC
//! channels.  There is no serialisation overhead beyond MessagePack (which
//! the runtime performs regardless of transport); frames arrive as
//! `Bytes` objects with zero copying.
use async_trait::async_trait;
use bytes::Bytes;
use saikuro_exec::mpsc;
use tracing::trace;

use crate::{
    error::{Result, TransportError},
    traits::{Transport, TransportReceiver, TransportSender},
};

/// Default channel capacity for in-memory transports.
///
/// This bounds memory usage and provides backpressure: if the receiver is
/// slow the sender's `send` call will yield until space frees up.
const DEFAULT_CHANNEL_CAPACITY: usize = 256;

/// An in-memory transport backed by a pair of MPSC channels.
///
/// Construct a connected pair with [`MemoryTransport::pair`].
pub struct MemoryTransport {
    sender: mpsc::Sender<Bytes>,
    receiver: mpsc::Receiver<Bytes>,
    label: String,
}

impl MemoryTransport {
    /// Create a connected pair of in-memory transports.
    ///
    /// The two returned transports can be split and handed to separate tasks;
    /// bytes sent on one will be received on the other.
    ///
    /// `label_a` and `label_b` are used only for log output.
    pub fn pair(label_a: impl Into<String>, label_b: impl Into<String>) -> (Self, Self) {
        let (a_tx, b_rx) = mpsc::channel(DEFAULT_CHANNEL_CAPACITY);
        let (b_tx, a_rx) = mpsc::channel(DEFAULT_CHANNEL_CAPACITY);

        let transport_a = Self {
            sender: a_tx,
            receiver: a_rx,
            label: label_a.into(),
        };
        let transport_b = Self {
            sender: b_tx,
            receiver: b_rx,
            label: label_b.into(),
        };

        (transport_a, transport_b)
    }

    /// Create a pair with the default labels `"client"` and `"server"`.
    pub fn connected_pair() -> (Self, Self) {
        Self::pair("client", "server")
    }
}

impl Transport for MemoryTransport {
    type Sender = MemorySender;
    type Receiver = MemoryReceiver;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        (
            MemorySender {
                inner: self.sender,
                label: self.label.clone(),
            },
            MemoryReceiver {
                inner: self.receiver,
                label: self.label,
            },
        )
    }

    fn description(&self) -> &str {
        "in-memory"
    }
}

/// Sending half of a [`MemoryTransport`].
pub struct MemorySender {
    inner: mpsc::Sender<Bytes>,
    label: String,
}

#[async_trait]
impl TransportSender for MemorySender {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        trace!(label = %self.label, bytes = frame.len(), "memory send");
        self.inner.send(frame).await.map_err(|_| {
            TransportError::ConnectionLost(format!(
                "in-memory receiver dropped for '{}'",
                self.label
            ))
        })
    }

    async fn close(&mut self) -> Result<()> {
        // Dropping the sender closes the channel; the receiver will see None.
        // There is nothing explicit to do here:  the sender will be dropped
        // when this struct is dropped.
        trace!(label = %self.label, "memory sender closing");
        Ok(())
    }
}

/// Receiving half of a [`MemoryTransport`].
pub struct MemoryReceiver {
    inner: mpsc::Receiver<Bytes>,
    label: String,
}

#[async_trait]
impl TransportReceiver for MemoryReceiver {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        let result = self.inner.recv().await;
        match &result {
            Some(bytes) => trace!(label = %self.label, bytes = bytes.len(), "memory recv"),
            None => trace!(label = %self.label, "memory channel closed"),
        }
        Ok(result)
    }
}
