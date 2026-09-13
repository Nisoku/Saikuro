use alloc::boxed::Box;
use alloc::string::String;
use async_trait::async_trait;
use bytes::Bytes;

use saikuro_transport::shared::error::Result;
use saikuro_transport::shared::host::{HostPipeFactory, Role, WasmHostTransport};
use saikuro_transport::shared::traits::{
    LocalTransport, LocalTransportListener, LocalTransportReceiver, LocalTransportSender,
    Transport, TransportListener, TransportReceiver, TransportSender,
};

mod nosend_runtime_traits {
    use super::*;

    #[async_trait(?Send)]
    pub trait RuntimeSender {
        async fn send(&mut self, frame: Bytes) -> Result<()>;
        async fn close(&mut self) -> Result<()>;
    }
    #[async_trait(?Send)]
    pub trait RuntimeReceiver {
        async fn recv(&mut self) -> Result<Option<Bytes>>;
    }
    #[async_trait(?Send)]
    pub trait RuntimeTransport {
        type Sender: RuntimeSender + 'static;
        type Receiver: RuntimeReceiver + 'static;
        fn split(self) -> (Self::Sender, Self::Receiver);
        fn description(&self) -> &str;
    }
    #[async_trait(?Send)]
    pub trait RuntimeListener {
        type Output: RuntimeTransport + 'static;
        async fn accept(&mut self) -> Result<Option<Self::Output>>;
        async fn close(&mut self) -> Result<()>;
    }
}

pub use nosend_runtime_traits::*;

// Blanket impls forwarding the `Transport*` family to the runtime traits.
#[async_trait(?Send)]
impl<T: TransportSender> RuntimeSender for T {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        T::send(self, frame).await
    }
    async fn close(&mut self) -> Result<()> {
        T::close(self).await
    }
}

#[async_trait(?Send)]
impl<T: TransportReceiver> RuntimeReceiver for T {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        T::recv(self).await
    }
}

#[async_trait(?Send)]
impl<T: Transport + 'static> RuntimeTransport for T {
    type Sender = T::Sender;
    type Receiver = T::Receiver;
    fn split(self) -> (Self::Sender, Self::Receiver) {
        T::split(self)
    }
    fn description(&self) -> &str {
        T::description(self)
    }
}

#[async_trait(?Send)]
impl<T: TransportListener + 'static> RuntimeListener for T {
    type Output = T::Output;
    async fn accept(&mut self) -> Result<Option<Self::Output>> {
        T::accept(self).await
    }
    async fn close(&mut self) -> Result<()> {
        T::close(self).await
    }
}

// Non-native engines adapt the `LocalTransport*` family into the runtime traits
// via these wrappers.
pub struct LocalRuntimeSender<S: LocalTransportSender>(S);

#[async_trait(?Send)]
impl<S: LocalTransportSender> RuntimeSender for LocalRuntimeSender<S> {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        self.0.send(frame).await
    }
    async fn close(&mut self) -> Result<()> {
        self.0.close().await
    }
}

pub struct LocalRuntimeReceiver<R: LocalTransportReceiver>(R);

#[async_trait(?Send)]
impl<R: LocalTransportReceiver> RuntimeReceiver for LocalRuntimeReceiver<R> {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        self.0.recv().await
    }
}

pub struct LocalRuntimeTransport<T: LocalTransport>(T);

#[async_trait(?Send)]
impl<T: LocalTransport + 'static> RuntimeTransport for LocalRuntimeTransport<T>
where
    T::Sender: 'static,
    T::Receiver: 'static,
{
    type Sender = LocalRuntimeSender<T::Sender>;
    type Receiver = LocalRuntimeReceiver<T::Receiver>;
    fn split(self) -> (Self::Sender, Self::Receiver) {
        let (sender, receiver) = self.0.split();
        (LocalRuntimeSender(sender), LocalRuntimeReceiver(receiver))
    }
    fn description(&self) -> &str {
        self.0.description()
    }
}

pub struct LocalRuntimeListener<L: LocalTransportListener>(L);

impl<L: LocalTransportListener> LocalRuntimeListener<L> {
    /// Wrap a `LocalTransportListener` so it satisfies [`RuntimeListener`].
    pub fn new(listener: L) -> Self {
        Self(listener)
    }
}

#[async_trait(?Send)]
impl<L: LocalTransportListener + 'static> RuntimeListener for LocalRuntimeListener<L>
where
    L::Output: 'static,
    <L::Output as LocalTransport>::Sender: 'static,
    <L::Output as LocalTransport>::Receiver: 'static,
{
    type Output = LocalRuntimeTransport<L::Output>;
    async fn accept(&mut self) -> Result<Option<Self::Output>> {
        match L::accept(&mut self.0).await? {
            Some(transport) => Ok(Some(LocalRuntimeTransport(transport))),
            None => Ok(None),
        }
    }
    async fn close(&mut self) -> Result<()> {
        L::close(&mut self.0).await
    }
}

pub struct HostPipeListener<F: HostPipeFactory> {
    channel: String,
    _marker: core::marker::PhantomData<fn() -> F>,
}

impl<F: HostPipeFactory> HostPipeListener<F> {
    /// Start listening for a rendezvous connection on `channel`.
    pub fn new(channel: impl Into<String>) -> Self {
        Self {
            channel: channel.into(),
            _marker: core::marker::PhantomData,
        }
    }
}

#[async_trait(?Send)]
impl<F: HostPipeFactory + 'static> RuntimeListener for HostPipeListener<F>
where
    F::Send: 'static,
    F::Recv: 'static,
{
    type Output = LocalRuntimeTransport<WasmHostTransport<F::Send, F::Recv>>;
    async fn accept(&mut self) -> Result<Option<Self::Output>> {
        let (send, recv) = F::open(&self.channel, Role::Accept).await?;
        let transport = WasmHostTransport::new(send, recv);
        Ok(Some(LocalRuntimeTransport(transport)))
    }
    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}
