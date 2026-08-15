use crate::{impl_native_receiver, impl_native_sender};
use async_trait::async_trait;
use bytes::Bytes;
use saikuro_net::net::{TcpListener, TcpStream};
use std::net::SocketAddr;
use tracing::debug;

use crate::shared::{
    error::Result,
    framing::FramedStream,
    traits::{Transport, TransportConnector, TransportListener},
};

/// A TCP transport connection.
pub struct TcpTransport {
    framed: FramedStream<TcpStream>,
    peer_addr: SocketAddr,
}

impl TcpTransport {
    /// Wrap an already-connected [`TcpStream`].
    pub fn new(stream: TcpStream) -> Result<Self> {
        let peer_addr = stream.peer_addr()?;
        // Disable Nagle's algorithm: Saikuro sends complete frames and latency
        // matters more than segment coalescing.
        stream.set_nodelay(true)?;
        Ok(Self {
            framed: FramedStream::new(stream),
            peer_addr,
        })
    }
}

impl Transport for TcpTransport {
    type Sender = TcpSender;
    type Receiver = TcpReceiver;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        let peer = self.peer_addr;
        let (sink, stream) = self.framed.split();
        (
            TcpSender {
                inner: sink,
                peer_addr: peer,
            },
            TcpReceiver {
                inner: stream,
                peer_addr: peer,
            },
        )
    }

    fn description(&self) -> &str {
        "tcp"
    }
}

// Sender / Receiver
pub struct TcpSender {
    inner: futures::stream::SplitSink<FramedStream<TcpStream>, Bytes>,
    peer_addr: SocketAddr,
}

impl_native_sender!(TcpSender, peer_addr, "tcp");

pub struct TcpReceiver {
    inner: futures::stream::SplitStream<FramedStream<TcpStream>>,
    peer_addr: SocketAddr,
}

impl_native_receiver!(TcpReceiver, peer_addr, "tcp");

/// Establishes outgoing TCP connections.
pub struct TcpConnector {
    addr: SocketAddr,
}

impl TcpConnector {
    pub fn new(addr: SocketAddr) -> Self {
        Self { addr }
    }
}

#[async_trait]
impl TransportConnector for TcpConnector {
    type Output = TcpTransport;

    async fn connect(&self) -> Result<Self::Output> {
        debug!(addr = %self.addr, "tcp connecting");
        let stream = TcpStream::connect(self.addr).await?;
        TcpTransport::new(stream)
    }
}

/// Accepts incoming TCP connections.
pub struct TcpTransportListener {
    inner: TcpListener,
    local_addr: SocketAddr,
}

impl TcpTransportListener {
    /// Bind a listener on the given address.
    pub async fn bind(addr: SocketAddr) -> Result<Self> {
        let inner = TcpListener::bind(addr).await?;
        let local_addr = inner.local_addr()?;
        debug!(%local_addr, "tcp listener bound");
        Ok(Self { inner, local_addr })
    }

    /// Return the address this listener is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

#[async_trait]
impl TransportListener for TcpTransportListener {
    type Output = TcpTransport;

    async fn accept(&mut self) -> Result<Option<Self::Output>> {
        match self.inner.accept().await {
            Ok((stream, _peer)) => {
                debug!(peer = %_peer, "tcp accepted connection");
                Ok(Some(TcpTransport::new(stream)?))
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn close(&mut self) -> Result<()> {
        debug!(local = %self.local_addr, "tcp listener closing");
        // TcpListener closes on drop.
        Ok(())
    }
}
