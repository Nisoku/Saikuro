use async_trait::async_trait;
use bytes::Bytes;

use saikuro_transport::shared::error::Result;
use saikuro_transport::shared::traits::{
    Transport, TransportListener, TransportReceiver, TransportSender,
};

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

pub use send_runtime_traits::*;

// Blanket impls forwarding the `Transport*` family to the runtime traits.
#[async_trait]
impl<T: TransportSender> RuntimeSender for T {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        T::send(self, frame).await
    }
    async fn close(&mut self) -> Result<()> {
        T::close(self).await
    }
}

#[async_trait]
impl<T: TransportReceiver> RuntimeReceiver for T {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        T::recv(self).await
    }
}

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
