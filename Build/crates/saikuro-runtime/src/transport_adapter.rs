use alloc::boxed::Box;
use alloc::string::String;
use async_trait::async_trait;
use bytes::Bytes;

use saikuro_transport::shared::error::Result;
use saikuro_transport::shared::host::{HostPipeFactory, Role, WasmHostTransport};
use saikuro_transport::shared::traits::{
    LocalTransport, LocalTransportListener, LocalTransportReceiver, LocalTransportSender, Transport,
    TransportListener, TransportReceiver, TransportSender,
};

macro_rules! define_runtime_traits {
    (SEND) => {
        #[async_trait]
        pub trait RuntimeSender: Send + Sync {
            async fn send(&mut self, frame: Bytes) -> Result<()>;
            async fn close(&mut self) -> Result<()>;
        }
        #[async_trait]
        pub trait RuntimeReceiver: Send + Sync {
            async fn recv(&mut self) -> Result<Option<Bytes>>;
        }
        #[async_trait]
        pub trait RuntimeTransport: Send + Sync {
            type Sender: RuntimeSender;
            type Receiver: RuntimeReceiver;
            fn split(self) -> (Self::Sender, Self::Receiver);
            fn description(&self) -> &str;
        }
        #[async_trait]
        pub trait RuntimeListener: Send + Sync {
            /// The concrete transport produced by a successful accept.
            type Output: RuntimeTransport;
            async fn accept(&mut self) -> Result<Option<Self::Output>>;
            async fn close(&mut self) -> Result<()>;
        }
    };
    (NOSEND) => {
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
            type Sender: RuntimeSender;
            type Receiver: RuntimeReceiver;
            fn split(self) -> (Self::Sender, Self::Receiver);
            fn description(&self) -> &str;
        }
        #[async_trait(?Send)]
        pub trait RuntimeListener {
            type Output: RuntimeTransport;
            async fn accept(&mut self) -> Result<Option<Self::Output>>;
            async fn close(&mut self) -> Result<()>;
        }
    };
}

#[cfg(feature = "native")]
define_runtime_traits!(SEND);
#[cfg(not(feature = "native"))]
define_runtime_traits!(NOSEND);

// Blanket impls for the boxed (native) family.

#[cfg_attr(feature = "native", async_trait)]
#[cfg_attr(not(feature = "native"), async_trait(?Send))]
impl<T: TransportSender> RuntimeSender for T {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        T::send(self, frame).await
    }
    async fn close(&mut self) -> Result<()> {
        T::close(self).await
    }
}

#[cfg_attr(feature = "native", async_trait)]
#[cfg_attr(not(feature = "native"), async_trait(?Send))]
impl<T: TransportReceiver> RuntimeReceiver for T {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        T::recv(self).await
    }
}

#[cfg_attr(feature = "native", async_trait)]
#[cfg_attr(not(feature = "native"), async_trait(?Send))]
impl<T: Transport> RuntimeTransport for T {
    type Sender = T::Sender;
    type Receiver = T::Receiver;
    fn split(self) -> (Self::Sender, Self::Receiver) {
        (*self).split()
    }
    fn description(&self) -> &str {
        T::description(self)
    }
}

#[cfg_attr(feature = "native", async_trait)]
#[cfg_attr(not(feature = "native"), async_trait(?Send))]
impl<T: TransportListener> RuntimeListener for T {
    type Output = T::Output;
    async fn accept(&mut self) -> Result<Option<Self::Output>> {
        T::accept(self).await
    }
    async fn close(&mut self) -> Result<()> {
        T::close(self).await
    }
}


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
impl<T: LocalTransport> RuntimeTransport for LocalRuntimeTransport<T> {
    type Sender = LocalRuntimeSender<T::Sender>;
    type Receiver = LocalRuntimeReceiver<T::Receiver>;
    fn split(self) -> (Self::Sender, Self::Receiver) {
        let (sender, receiver) = (*self).0.split();
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
impl<L: LocalTransportListener> RuntimeListener for LocalRuntimeListener<L> {
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
/// [`RuntimeListener`].
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
impl<F: HostPipeFactory> RuntimeListener for HostPipeListener<F> {
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
