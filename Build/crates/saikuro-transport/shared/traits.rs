use alloc::boxed::Box;
use async_trait::async_trait;
use bytes::Bytes;

use crate::shared::error::Result;

// Exactly one engine is active per build (enforced by the facade crates), so we
// can define the transport traits with different auto-trait bounds per engine.
#[cfg(feature = "native")]
mod send_traits {
    use super::*;

    /// A bidirectional message transport.
    ///
    /// The runtime creates a [`Transport`] and splits it into a
    /// [`TransportSender`] and [`TransportReceiver`] pair, each of which can be
    /// moved to a separate task.  Messages are raw byte frames; framing/length
    /// prefixing is handled inside the concrete implementation.
    ///
    /// ## Implementation contract
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
        /// The ready transport produced by [`connect`](TransportConnector::connect).
        type Output: Transport;

        /// Establish a new connection, returning a ready [`Transport`].
        async fn connect(&self) -> Result<Self::Output>;
    }

    /// A listener that accepts inbound connections and produces transports.
    ///
    /// This is used by provider adapters and the runtime's listener loop.
    #[async_trait]
    pub trait TransportListener: Send + Sync + 'static {
        /// The ready transport produced by [`accept`](TransportListener::accept).
        type Output: Transport;

        /// Accept the next inbound connection.
        ///
        /// Returns `Ok(None)` when the listener has been shut down.
        async fn accept(&mut self) -> Result<Option<Self::Output>>;

        /// Stop accepting new connections.
        async fn close(&mut self) -> Result<()>;
    }

    /// Mirrors [`TransportSender`] but uses statically-dispatched local transports.
    #[async_trait]
    pub trait LocalTransportSender: Send {
        /// Send a single binary frame to the remote peer.
        async fn send(&mut self, frame: Bytes) -> Result<()>;
        /// Close the sending side gracefully.
        async fn close(&mut self) -> Result<()>;
    }

    /// A local, statically-dispatched receiving half of a transport.
    #[async_trait]
    pub trait LocalTransportReceiver: Send {
        /// Wait for and return the next binary frame, or `None` on clean close.
        async fn recv(&mut self) -> Result<Option<Bytes>>;
    }

    /// A local, statically-dispatched bidirectional transport.
    #[async_trait]
    pub trait LocalTransport: Send {
        /// The sender half type produced by [`split`](LocalTransport::split).
        type Sender: LocalTransportSender;
        /// The receiver half type produced by [`split`](LocalTransport::split).
        type Receiver: LocalTransportReceiver;

        /// Split into concurrently-usable sender and receiver halves.
        fn split(self) -> (Self::Sender, Self::Receiver);

        /// A human-readable description of the transport for logging.
        fn description(&self) -> &str;
    }

    /// A local factory that connects to a peer.
    #[async_trait]
    pub trait LocalTransportConnector: Send {
        /// The ready transport produced by [`connect`](LocalTransportConnector::connect).
        type Output: LocalTransport;

        /// Establish a new connection.
        async fn connect(&self) -> Result<Self::Output>;
    }

    /// A local listener that accepts inbound connections.
    #[async_trait]
    pub trait LocalTransportListener: Send {
        /// The ready transport produced by [`accept`](LocalTransportListener::accept).
        type Output: LocalTransport;

        /// Accept the next inbound connection, or `None` when shut down.
        async fn accept(&mut self) -> Result<Option<Self::Output>>;

        /// Stop accepting new connections.
        async fn close(&mut self) -> Result<()>;
    }
}

#[cfg(not(feature = "native"))]
mod nosend_traits {
    use super::*;

    /// A bidirectional message transport (embedded, single-threaded, `!Send`).
    #[async_trait(?Send)]
    pub trait Transport: 'static {
        type Sender: TransportSender;
        type Receiver: TransportReceiver;

        fn split(self) -> (Self::Sender, Self::Receiver);
        fn description(&self) -> &str;
    }

    #[async_trait(?Send)]
    pub trait TransportSender: 'static {
        async fn send(&mut self, frame: Bytes) -> Result<()>;
        async fn close(&mut self) -> Result<()>;
    }

    #[async_trait(?Send)]
    pub trait TransportReceiver: 'static {
        async fn recv(&mut self) -> Result<Option<Bytes>>;
    }

    #[async_trait(?Send)]
    pub trait TransportConnector: 'static {
        type Output: Transport;
        async fn connect(&self) -> Result<Self::Output>;
    }

    #[async_trait(?Send)]
    pub trait TransportListener: 'static {
        type Output: Transport;
        async fn accept(&mut self) -> Result<Option<Self::Output>>;
        async fn close(&mut self) -> Result<()>;
    }

    #[async_trait(?Send)]
    pub trait LocalTransportSender: 'static {
        async fn send(&mut self, frame: Bytes) -> Result<()>;
        async fn close(&mut self) -> Result<()>;
    }

    #[async_trait(?Send)]
    pub trait LocalTransportReceiver: 'static {
        async fn recv(&mut self) -> Result<Option<Bytes>>;
    }

    #[async_trait(?Send)]
    pub trait LocalTransport: 'static {
        type Sender: LocalTransportSender;
        type Receiver: LocalTransportReceiver;
        fn split(self) -> (Self::Sender, Self::Receiver);
        fn description(&self) -> &str;
    }

    #[async_trait(?Send)]
    pub trait LocalTransportConnector: 'static {
        type Output: LocalTransport;
        async fn connect(&self) -> Result<Self::Output>;
    }

    #[async_trait(?Send)]
    pub trait LocalTransportListener: 'static {
        type Output: LocalTransport;
        async fn accept(&mut self) -> Result<Option<Self::Output>>;
        async fn close(&mut self) -> Result<()>;
    }
}

#[cfg(feature = "native")]
pub use send_traits::*;

#[cfg(not(feature = "native"))]
pub use nosend_traits::*;
