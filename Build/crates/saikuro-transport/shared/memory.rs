use alloc::boxed::Box;
use alloc::string::String;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;
use async_trait::async_trait;
use bytes::Bytes;
#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic_util::Arc;
use saikuro_event::{LogLevel, LogRecord};
use saikuro_exec::mpsc;

use crate::shared::{
    error::{Result, TransportError},
    traits::{Transport, TransportReceiver, TransportSender},
};

use crate::DEFAULT_CHANNEL_CAPACITY;

/// An in-memory transport backed by a pair of MPSC channels.
///
/// Construct a connected pair with [`MemoryTransport::pair`].
pub struct MemoryTransport {
    sender: mpsc::Sender<Bytes>,
    receiver: mpsc::Receiver<Bytes>,
    label: String,
    log: Arc<dyn saikuro_event::LogSink>,
}

impl MemoryTransport {
    /// Create a connected pair of in-memory transports.
    ///
    /// The two returned transports can be split and handed to separate tasks;
    /// bytes sent on one will be received on the other.
    ///
    /// `label_a` and `label_b` are used only for log output.
    pub fn pair(
        label_a: impl Into<String>,
        label_b: impl Into<String>,
        log: Arc<dyn saikuro_event::LogSink>,
    ) -> (Self, Self) {
        let (a_tx, b_rx) = mpsc::channel(DEFAULT_CHANNEL_CAPACITY);
        let (b_tx, a_rx) = mpsc::channel(DEFAULT_CHANNEL_CAPACITY);

        let transport_a = Self {
            sender: a_tx,
            receiver: a_rx,
            label: label_a.into(),
            log: log.clone(),
        };
        let transport_b = Self {
            sender: b_tx,
            receiver: b_rx,
            label: label_b.into(),
            log,
        };

        (transport_a, transport_b)
    }

    /// Create a pair with the default labels `"client"` and `"server"`.
    pub fn connected_pair(log: Arc<dyn saikuro_event::LogSink>) -> (Self, Self) {
        Self::pair("client", "server", log)
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
                log: self.log.clone(),
            },
            MemoryReceiver {
                inner: self.receiver,
                label: self.label,
                log: self.log,
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
    log: Arc<dyn saikuro_event::LogSink>,
}

/// Receiving half of a [`MemoryTransport`].
pub struct MemoryReceiver {
    inner: mpsc::Receiver<Bytes>,
    label: String,
    log: Arc<dyn saikuro_event::LogSink>,
}

#[cfg(feature = "native")]
mod send_impls {
    use super::*;

    #[async_trait]
    impl TransportSender for MemorySender {
        async fn send(&mut self, frame: Bytes) -> Result<()> {
            let mut record =
                LogRecord::now(LogLevel::Trace, "saikuro.transport.memory", "memory send");
            record.set_context("label", self.label.clone());
            record.set_context("bytes", frame.len() as u64);
            self.log.emit(&record).await;
            self.inner.send(frame).await.map_err(|_| {
                TransportError::ConnectionLost(format!(
                    "in-memory receiver dropped for '{}'",
                    self.label
                ))
            })
        }

        async fn close(&mut self) -> Result<()> {
            let mut record = LogRecord::now(
                LogLevel::Trace,
                "saikuro.transport.memory",
                "memory sender closing",
            );
            record.set_context("label", self.label.clone());
            self.log.emit(&record).await;
            Ok(())
        }
    }

    #[async_trait]
    impl TransportReceiver for MemoryReceiver {
        async fn recv(&mut self) -> Result<Option<Bytes>> {
            let result = self.inner.recv().await;
            match &result {
                Some(bytes) => {
                    let mut record =
                        LogRecord::now(LogLevel::Trace, "saikuro.transport.memory", "memory recv");
                    record.set_context("label", self.label.clone());
                    record.set_context("bytes", bytes.len() as u64);
                    self.log.emit(&record).await;
                }
                None => {
                    let mut record = LogRecord::now(
                        LogLevel::Trace,
                        "saikuro.transport.memory",
                        "memory channel closed",
                    );
                    record.set_context("label", self.label.clone());
                    self.log.emit(&record).await;
                }
            }
            Ok(result)
        }
    }
}

#[cfg(not(feature = "native"))]
mod nosend_impls {
    use super::*;

    #[async_trait(?Send)]
    impl TransportSender for MemorySender {
        async fn send(&mut self, frame: Bytes) -> Result<()> {
            let mut record =
                LogRecord::now(LogLevel::Trace, "saikuro.transport.memory", "memory send");
            record.set_context("label", self.label.clone());
            record.set_context("bytes", frame.len() as u64);
            self.log.emit(&record).await;
            self.inner.send(frame).await.map_err(|_| {
                TransportError::ConnectionLost(format!(
                    "in-memory receiver dropped for '{}'",
                    self.label
                ))
            })
        }

        async fn close(&mut self) -> Result<()> {
            let mut record = LogRecord::now(
                LogLevel::Trace,
                "saikuro.transport.memory",
                "memory sender closing",
            );
            record.set_context("label", self.label.clone());
            self.log.emit(&record).await;
            Ok(())
        }
    }

    #[async_trait(?Send)]
    impl TransportReceiver for MemoryReceiver {
        async fn recv(&mut self) -> Result<Option<Bytes>> {
            let result = self.inner.recv().await;
            match &result {
                Some(bytes) => {
                    let mut record =
                        LogRecord::now(LogLevel::Trace, "saikuro.transport.memory", "memory recv");
                    record.set_context("label", self.label.clone());
                    record.set_context("bytes", bytes.len() as u64);
                    self.log.emit(&record).await;
                }
                None => {
                    let mut record = LogRecord::now(
                        LogLevel::Trace,
                        "saikuro.transport.memory",
                        "memory channel closed",
                    );
                    record.set_context("label", self.label.clone());
                    self.log.emit(&record).await;
                }
            }
            Ok(result)
        }
    }
}
