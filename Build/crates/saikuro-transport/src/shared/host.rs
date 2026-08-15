use alloc::string::String;
use alloc::vec::Vec;
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

/// The sending half of a host message bus, abstracted over its backend.
pub trait HostPipeSend {
    /// Send a single binary frame over the bus.
    async fn send(&mut self, frame: &[u8]) -> Result<()>;
}

/// The receiving half of a host message bus, abstracted over its backend.
pub trait HostPipeRecv {
    /// Receive the next binary frame, or `None` when the peer closed cleanly.
    async fn recv(&mut self) -> Result<Option<Vec<u8>>>;
}

/// A host message bus that can be opened as a connected, framed pipe.
pub trait HostPipeFactory {
    /// The sending half produced by [`open`](HostPipeFactory::open).
    type Send: HostPipeSend;
    /// The receiving half produced by [`open`](HostPipeFactory::open).
    type Recv: HostPipeRecv;

    /// Open a connected pipe on `channel` playing `role`.
    async fn open(channel: &str, role: Role) -> Result<(Self::Send, Self::Recv)>;
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
            WasmHostSender {
                pipe: self.sender,
            },
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

impl<S: HostPipeSend> LocalTransportSender for WasmHostSender<S> {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        self.pipe.send(&frame).await
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Receiving half of a [`WasmHostTransport`].
pub struct WasmHostReceiver<R> {
    pipe: R,
}

impl<R: HostPipeRecv> LocalTransportReceiver for WasmHostReceiver<R> {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        match self.pipe.recv().await? {
            Some(bytes) => Ok(Some(Bytes::from(bytes))),
            None => Ok(None),
        }
    }
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

impl<F: HostPipeFactory> LocalTransportConnector for WasmHostConnector<F> {
    type Output = WasmHostTransport<F::Send, F::Recv>;

    async fn connect(&self) -> Result<Self::Output> {
        let (sender, receiver) = F::open(&self.channel, Role::Connect).await?;
        Ok(WasmHostTransport::new(sender, receiver))
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
