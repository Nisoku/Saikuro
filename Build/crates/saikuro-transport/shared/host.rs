use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use async_trait::async_trait;
use bytes::Bytes;
use core::marker::PhantomData;

use crate::shared::error::Result;
use crate::shared::traits::{
    LocalTransport, LocalTransportConnector, LocalTransportListener, LocalTransportReceiver,
    LocalTransportSender,
};

/// Which side of a rendezvous a pipe endpoint plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The active side: dials/reaches out and waits for an accept.
    Connect,
    /// The passive side: listens and replies with an accept.
    Accept,
}

/// A transport backed by a host message bus, generic over its pipe backend.
pub struct WasmHostTransport<S, R> {
    sender: S,
    receiver: R,
}

impl<S: HostPipeSend, R: HostPipeRecv> WasmHostTransport<S, R> {
    /// Wrap an already-open pipe into a transport.
    pub fn new(sender: S, receiver: R) -> Self {
        Self { sender, receiver }
    }
}

impl<S: HostPipeSend, R: HostPipeRecv> LocalTransport for WasmHostTransport<S, R> {
    type Sender = WasmHostSender<S>;
    type Receiver = WasmHostReceiver<R>;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        (
            WasmHostSender { pipe: self.sender },
            WasmHostReceiver {
                pipe: self.receiver,
            },
        )
    }

    fn description(&self) -> &str {
        "wasm-host"
    }
}

/// Sending half of a [`WasmHostTransport`].
pub struct WasmHostSender<S> {
    pipe: S,
}

/// Receiving half of a [`WasmHostTransport`].
pub struct WasmHostReceiver<R> {
    pipe: R,
}

/// Connects to a peer over a host message bus.
pub struct WasmHostConnector<F> {
    channel: String,
    _marker: PhantomData<fn() -> F>,
}

impl<F: HostPipeFactory> WasmHostConnector<F> {
    /// Create a connector that will rendezvous on `channel`.
    pub fn new(channel: impl Into<String>) -> Self {
        Self {
            channel: channel.into(),
            _marker: PhantomData,
        }
    }
}

/// Accepts inbound connections over a host message bus.
pub struct WasmHostListener<F> {
    channel: String,
    _marker: PhantomData<fn() -> F>,
}

impl<F: HostPipeFactory> WasmHostListener<F> {
    /// Start listening for connections on `channel`.
    pub fn new(channel: impl Into<String>) -> Self {
        Self {
            channel: channel.into(),
            _marker: PhantomData,
        }
    }
}

// Exactly one engine is active per build, so the host-pipe traits carry
// different auto-trait bounds per engine (matching `shared::traits`).
#[cfg(feature = "native")]
mod send_traits {
    use super::*;

    /// The sending half of a host message bus, abstracted over its backend.
    #[async_trait]
    pub trait HostPipeSend: Send + 'static {
        /// Send a single binary frame over the bus.
        async fn send(&mut self, frame: &[u8]) -> Result<()>;
        /// Close the sending side gracefully, signaling EOF to the peer.
        async fn close(&mut self) -> Result<()>;
    }

    /// The receiving half of a host message bus, abstracted over its backend.
    #[async_trait]
    pub trait HostPipeRecv: Send + 'static {
        /// Receive the next binary frame, or `None` on a clean peer close.
        async fn recv(&mut self) -> Result<Option<Vec<u8>>>;
    }

    /// A host message bus that can be opened as a connected, framed pipe.
    #[async_trait]
    pub trait HostPipeFactory: Send + 'static {
        /// The sending half produced by [`open`](HostPipeFactory::open).
        type Send: HostPipeSend;
        /// The receiving half produced by [`open`](HostPipeFactory::open).
        type Recv: HostPipeRecv;

        /// Open a connected pipe on `channel` playing `role`.
        async fn open(channel: &str, role: Role) -> Result<(Self::Send, Self::Recv)>;
    }

    #[async_trait]
    impl<S: HostPipeSend + Send> LocalTransportSender for WasmHostSender<S> {
        async fn send(&mut self, frame: Bytes) -> Result<()> {
            self.pipe.send(&frame).await
        }

        async fn close(&mut self) -> Result<()> {
            self.pipe.close().await
        }
    }

    #[async_trait]
    impl<R: HostPipeRecv + Send> LocalTransportReceiver for WasmHostReceiver<R> {
        async fn recv(&mut self) -> Result<Option<Bytes>> {
            match self.pipe.recv().await? {
                Some(bytes) => Ok(Some(Bytes::from(bytes))),
                None => Ok(None),
            }
        }
    }

    #[async_trait]
    impl<F: HostPipeFactory + Send> LocalTransportConnector for WasmHostConnector<F> {
        type Output = WasmHostTransport<F::Send, F::Recv>;

        async fn connect(&self) -> Result<Self::Output> {
            let (sender, receiver) = F::open(&self.channel, Role::Connect).await?;
            Ok(WasmHostTransport::new(sender, receiver))
        }
    }

    #[async_trait]
    impl<F: HostPipeFactory + Send> LocalTransportListener for WasmHostListener<F> {
        type Output = WasmHostTransport<F::Send, F::Recv>;

        async fn accept(&mut self) -> Result<Option<Self::Output>> {
            let (sender, receiver) = F::open(&self.channel, Role::Accept).await?;
            Ok(Some(WasmHostTransport::new(sender, receiver)))
        }

        async fn close(&mut self) -> Result<()> {
            Ok(())
        }
    }
}

#[cfg(not(feature = "native"))]
mod nosend_traits {
    use super::*;

    /// The sending half of a host message bus, abstracted over its backend.
    #[async_trait(?Send)]
    pub trait HostPipeSend: 'static {
        /// Send a single binary frame over the bus.
        async fn send(&mut self, frame: &[u8]) -> Result<()>;
        /// Close the sending side gracefully, signaling EOF to the peer.
        async fn close(&mut self) -> Result<()>;
    }

    /// The receiving half of a host message bus, abstracted over its backend.
    #[async_trait(?Send)]
    pub trait HostPipeRecv: 'static {
        /// Receive the next binary frame, or `None` on a clean peer close.
        async fn recv(&mut self) -> Result<Option<Vec<u8>>>;
    }

    /// A host message bus that can be opened as a connected, framed pipe.
    #[async_trait(?Send)]
    pub trait HostPipeFactory: 'static {
        /// The sending half produced by [`open`](HostPipeFactory::open).
        type Send: HostPipeSend;
        /// The receiving half produced by [`open`](HostPipeFactory::open).
        type Recv: HostPipeRecv;

        /// Open a connected pipe on `channel` playing `role`.
        async fn open(channel: &str, role: Role) -> Result<(Self::Send, Self::Recv)>;
    }

    #[async_trait(?Send)]
    impl<S: HostPipeSend> LocalTransportSender for WasmHostSender<S> {
        async fn send(&mut self, frame: Bytes) -> Result<()> {
            self.pipe.send(&frame).await
        }

        async fn close(&mut self) -> Result<()> {
            self.pipe.close().await
        }
    }

    #[async_trait(?Send)]
    impl<R: HostPipeRecv> LocalTransportReceiver for WasmHostReceiver<R> {
        async fn recv(&mut self) -> Result<Option<Bytes>> {
            match self.pipe.recv().await? {
                Some(bytes) => Ok(Some(Bytes::from(bytes))),
                None => Ok(None),
            }
        }
    }

    #[async_trait(?Send)]
    impl<F: HostPipeFactory> LocalTransportConnector for WasmHostConnector<F> {
        type Output = WasmHostTransport<F::Send, F::Recv>;

        async fn connect(&self) -> Result<Self::Output> {
            let (sender, receiver) = F::open(&self.channel, Role::Connect).await?;
            Ok(WasmHostTransport::new(sender, receiver))
        }
    }

    #[async_trait(?Send)]
    impl<F: HostPipeFactory> LocalTransportListener for WasmHostListener<F> {
        type Output = WasmHostTransport<F::Send, F::Recv>;

        async fn accept(&mut self) -> Result<Option<Self::Output>> {
            let (sender, receiver) = F::open(&self.channel, Role::Accept).await?;
            Ok(Some(WasmHostTransport::new(sender, receiver)))
        }

        async fn close(&mut self) -> Result<()> {
            Ok(())
        }
    }
}

#[cfg(feature = "native")]
pub use send_traits::*;

#[cfg(not(feature = "native"))]
pub use nosend_traits::*;
