use alloc::boxed::Box;
#[cfg(not(feature = "native"))]
use alloc::string::String;
use async_trait::async_trait;
use bytes::Bytes;

use saikuro_transport::shared::error::Result;
#[cfg(not(feature = "native"))]
use saikuro_transport::shared::host::{HostPipeFactory, Role, WasmHostTransport};
#[cfg(not(feature = "native"))]
use saikuro_transport::shared::traits::{
    LocalTransport, LocalTransportListener, LocalTransportReceiver, LocalTransportSender,
};
use saikuro_transport::shared::traits::{
    Transport, TransportListener, TransportReceiver, TransportSender,
};

#[cfg(feature = "native")]
mod send_runtime_traits {
    use super::*;

    #[async_trait]
    pub trait RuntimeSender: Send {
        async fn send(&mut self, frame: Bytes) -> Result<()>;
        async fn close(&mut self) -> Result<()>;
    }
    #[async_trait]
    pub trait RuntimeReceiver: Send {
        async fn recv(&mut self) -> Result<Option<Bytes>>;
    }
    #[async_trait]
    pub trait RuntimeTransport: Send {
        type Sender: RuntimeSender + Send + Sync + 'static;
        type Receiver: RuntimeReceiver + Send + Sync + 'static;
        fn split(self) -> (Self::Sender, Self::Receiver);
        fn description(&self) -> &str;
    }
    #[async_trait]
    pub trait RuntimeListener: Send {
        type Output: RuntimeTransport + 'static;
        async fn accept(&mut self) -> Result<Option<Self::Output>>;
        async fn close(&mut self) -> Result<()>;
    }
}

#[cfg(not(feature = "native"))]
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

#[cfg(feature = "native")]
pub use send_runtime_traits::*;

#[cfg(not(feature = "native"))]
pub use nosend_runtime_traits::*;

// Blanket impls forwarding the `Transport*` family to the runtime traits.
// Native needs `Send` bounds (tokio tasks); non-native engines are `?Send`.
#[cfg(feature = "native")]
#[async_trait]
impl<T: TransportSender> RuntimeSender for T {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        T::send(self, frame).await
    }
    async fn close(&mut self) -> Result<()> {
        T::close(self).await
    }
}

#[cfg(feature = "native")]
#[async_trait]
impl<T: TransportReceiver> RuntimeReceiver for T {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        T::recv(self).await
    }
}

#[cfg(feature = "native")]
#[async_trait]
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

#[cfg(feature = "native")]
#[async_trait]
impl<T: TransportListener + 'static> RuntimeListener for T {
    type Output = T::Output;
    async fn accept(&mut self) -> Result<Option<Self::Output>> {
        T::accept(self).await
    }
    async fn close(&mut self) -> Result<()> {
        T::close(self).await
    }
}

#[cfg(not(feature = "native"))]
#[async_trait(?Send)]
impl<T: TransportSender> RuntimeSender for T {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        T::send(self, frame).await
    }
    async fn close(&mut self) -> Result<()> {
        T::close(self).await
    }
}

#[cfg(not(feature = "native"))]
#[async_trait(?Send)]
impl<T: TransportReceiver> RuntimeReceiver for T {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        T::recv(self).await
    }
}

#[cfg(not(feature = "native"))]
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

#[cfg(not(feature = "native"))]
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
// via these wrappers. Native does not use them: it forwards `Transport*` above.
#[cfg(not(feature = "native"))]
pub struct LocalRuntimeSender<S: LocalTransportSender>(S);

#[cfg(not(feature = "native"))]
#[async_trait(?Send)]
impl<S: LocalTransportSender> RuntimeSender for LocalRuntimeSender<S> {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        self.0.send(frame).await
    }
    async fn close(&mut self) -> Result<()> {
        self.0.close().await
    }
}

#[cfg(not(feature = "native"))]
pub struct LocalRuntimeReceiver<R: LocalTransportReceiver>(R);

#[cfg(not(feature = "native"))]
#[async_trait(?Send)]
impl<R: LocalTransportReceiver> RuntimeReceiver for LocalRuntimeReceiver<R> {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        self.0.recv().await
    }
}

#[cfg(not(feature = "native"))]
pub struct LocalRuntimeTransport<T: LocalTransport>(T);

#[cfg(not(feature = "native"))]
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

#[cfg(not(feature = "native"))]
pub struct LocalRuntimeListener<L: LocalTransportListener>(L);

#[cfg(not(feature = "native"))]
impl<L: LocalTransportListener> LocalRuntimeListener<L> {
    /// Wrap a `LocalTransportListener` so it satisfies [`RuntimeListener`].
    pub fn new(listener: L) -> Self {
        Self(listener)
    }
}

#[cfg(not(feature = "native"))]
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

/// Adapts a `HostPipeFactory` (BroadcastChannel / WASI loopback) into a
/// [`RuntimeListener`]-compatible listener. Only meaningful for the wasm / wasi
/// engines; the embedded engine uses its own embassy-net listener.
#[cfg(not(feature = "native"))]
pub struct HostPipeListener<F: HostPipeFactory> {
    channel: String,
    _marker: core::marker::PhantomData<fn() -> F>,
}

#[cfg(not(feature = "native"))]
impl<F: HostPipeFactory> HostPipeListener<F> {
    /// Start listening for a rendezvous connection on `channel`.
    pub fn new(channel: impl Into<String>) -> Self {
        Self {
            channel: channel.into(),
            _marker: core::marker::PhantomData,
        }
    }
}

#[cfg(not(feature = "native"))]
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
