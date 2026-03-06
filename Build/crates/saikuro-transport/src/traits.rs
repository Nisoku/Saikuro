//! Core transport traits.
//!
//! The runtime only ever talks to a [`Transport`], never to a specific
//! backend.  This makes it trivial to swap backends (e.g. from Unix socket
//! to WebSocket when moving to WASM) without touching any routing or
//! schema logic.

use async_trait::async_trait;
use bytes::Bytes;

use crate::error::Result;

/// A bidirectional message transport.
///
/// The runtime creates a [`Transport`] and splits it into a
/// [`TransportSender`] and [`TransportReceiver`] pair, each of which can be
/// moved to a separate task.  Messages are raw byte frames; framing/length
/// prefixing is handled inside the concrete implementation.
///
/// ## Implementation contract
///
/// - Implementations MUST guarantee ordered delivery within a connection.
/// - Implementations MUST be binary-safe (no newline stripping, etc.).
/// - Implementations SHOULD apply backpressure when internal send buffers fill.
/// - Implementations MUST be cancellation-safe on `send` and `recv`.
#[async_trait]
pub trait Transport: Send + Sync + 'static {
    /// The sender half type produced by [`split`](Transport::split).
    type Sender: TransportSender;
    /// The receiver half type produced by [`split`](Transport::split).
    type Receiver: TransportReceiver;

    /// Split this transport into a sender and receiver that can be used
    /// concurrently from separate tasks.
    fn split(self) -> (Self::Sender, Self::Receiver);

    /// A human-readable description of the transport for logging.
    fn description(&self) -> &str;
}

/// The sending half of a [`Transport`].
#[async_trait]
pub trait TransportSender: Send + Sync + 'static {
    /// Send a single binary frame to the remote peer.
    ///
    /// This method applies backpressure: if the send buffer is full it will
    /// yield the async task until space is available.
    async fn send(&mut self, frame: Bytes) -> Result<()>;

    /// Close the sending side gracefully.  Any frames already buffered will
    /// be flushed before the connection is terminated.
    async fn close(&mut self) -> Result<()>;
}

/// The receiving half of a [`Transport`].
#[async_trait]
pub trait TransportReceiver: Send + Sync + 'static {
    /// Wait for and return the next binary frame from the remote peer.
    ///
    /// Returns `Ok(None)` when the remote peer has closed the connection
    /// cleanly.  Returns `Err(_)` on unrecoverable transport errors.
    async fn recv(&mut self) -> Result<Option<Bytes>>;
}

/// A factory that can produce new [`Transport`] connections to a given peer.
///
/// This is the interface the runtime uses when it needs to connect to a
/// remote provider for the first time, or reconnect after a failure.
#[async_trait]
pub trait TransportConnector: Send + Sync + 'static {
    type Output: Transport;

    /// Establish a new connection, returning a ready [`Transport`].
    async fn connect(&self) -> Result<Self::Output>;
}

/// A listener that accepts inbound connections and produces transports.
///
/// This is used by provider adapters and the runtime's listener loop.
#[async_trait]
pub trait TransportListener: Send + Sync + 'static {
    type Output: Transport;

    /// Accept the next inbound connection.
    ///
    /// Returns `Ok(None)` when the listener has been shut down.
    async fn accept(&mut self) -> Result<Option<Self::Output>>;

    /// Stop accepting new connections.
    async fn close(&mut self) -> Result<()>;
}
