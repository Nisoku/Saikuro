use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use core::sync::Arc;
use embassy_sync::blocking_mutex::{CriticalSectionRawMutex, Mutex as BlockingMutex};
use tracing::debug;

use saikuro_net::net::{IpEndpoint, IpListenEndpoint, Stack, TcpSocket};

use crate::shared::framed::{read_exact, read_first_byte, write_all, HEADER_LEN};
use crate::shared::error::{Result, TransportError};
use crate::shared::traits::{
    Transport, TransportConnector, TransportListener, TransportReceiver, TransportSender,
};

type SharedSocket = Arc<BlockingMutex<CriticalSectionRawMutex, TcpSocket<'static>>>;

/// A TCP transport connection (embedded / embassy-net).
pub struct TcpTransport {
    socket: SharedSocket,
    peer: IpEndpoint,
}

impl TcpTransport {
    /// Wrap an already-connected embassy-net [`TcpSocket`].
    pub fn new(socket: TcpSocket<'static>, peer: IpEndpoint) -> Self {
        Self {
            socket: Arc::new(BlockingMutex::new(socket)),
            peer,
        }
    }
}

impl Transport for TcpTransport {
    type Sender = TcpSender;
    type Receiver = TcpReceiver;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        let socket = self.socket.clone();
        (
            TcpSender {
                socket: socket.clone(),
                peer: self.peer,
            },
            TcpReceiver {
                socket,
                peer: self.peer,
            },
        )
    }

    fn description(&self) -> &str {
        "tcp"
    }
}

/// Sending half of an embedded TCP transport.
pub struct TcpSender {
    socket: SharedSocket,
    peer: IpEndpoint,
}

#[async_trait]
impl TransportSender for TcpSender {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        if frame.len() > crate::MAX_FRAME_SIZE {
            return Err(TransportError::MessageTooLarge {
                size: frame.len(),
                limit: crate::MAX_FRAME_SIZE,
            });
        }
        let mut header = [0u8; HEADER_LEN];
        header.copy_from_slice(&(frame.len() as u32).to_be_bytes());
        let mut socket = self.socket.lock();
        write_all(&mut *socket, &header).await?;
        write_all(&mut *socket, &frame).await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Receiving half of an embedded TCP transport.
pub struct TcpReceiver {
    socket: SharedSocket,
    peer: IpEndpoint,
}

#[async_trait]
impl TransportReceiver for TcpReceiver {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        let mut socket = self.socket.lock();
        let mut header = [0u8; HEADER_LEN];
        if read_first_byte(&mut *socket, &mut header[0]).await? == 0 {
            return Ok(None);
        }
        read_exact(
            &mut *socket,
            &mut header[1..],
            "connection closed during frame header",
        )
        .await?;
        let frame_len = u32::from_be_bytes(header) as usize;
        if frame_len > crate::MAX_FRAME_SIZE {
            return Err(TransportError::MessageTooLarge {
                size: frame_len,
                limit: crate::MAX_FRAME_SIZE,
            });
        }
        let mut payload = BytesMut::zeroed(frame_len);
        read_exact(
            &mut *socket,
            &mut payload,
            "connection closed during frame payload",
        )
        .await?;
        Ok(Some(payload.freeze()))
    }
}

/// Establishes outgoing TCP connections over embassy-net.
pub struct TcpConnector {
    stack: &'static Stack<'static>,
    remote: IpEndpoint,
}

impl TcpConnector {
    /// Create a connector bound to `stack` targeting `remote`.
    pub fn new(stack: &'static Stack<'static>, remote: IpEndpoint) -> Self {
        Self { stack, remote }
    }
}

#[async_trait]
impl TransportConnector for TcpConnector {
    type Output = TcpTransport;

    async fn connect(&self) -> Result<Self::Output> {
        debug!(remote = ?self.remote, "embedded tcp connecting");
        let socket = TcpSocket::connect(self.stack, self.remote)
            .await
            .map_err(|e| TransportError::ConnectionRefused(format!("tcp connect failed: {e:?}")))?;
        let peer = socket.remote_endpoint().unwrap_or(self.remote);
        Ok(TcpTransport::new(socket, peer))
    }
}

/// Accepts incoming TCP connections over embassy-net.
///
/// Each accepted connection yields a fresh [`TcpSocket`] on the shared stack.
pub struct TcpTransportListener {
    stack: &'static Stack<'static>,
    local: IpEndpoint,
}

impl TcpTransportListener {
    /// Create a listener bound to `local` of `stack`.
    pub fn new(stack: &'static Stack<'static>, local: IpEndpoint) -> Self {
        Self { stack, local }
    }

    /// Return the endpoint this listener is bound to.
    pub fn local_addr(&self) -> IpEndpoint {
        self.local
    }
}

#[async_trait]
impl TransportListener for TcpTransportListener {
    type Output = TcpTransport;

    async fn accept(&mut self) -> Result<Option<Self::Output>> {
        // embassy-net has no TcpListener:  spin up a socket, put it in
        // listening mode, and await the single connection it accepts.
        let mut socket = TcpSocket::new(self.stack);
        socket
            .accept(IpListenEndpoint {
                addr: Some(self.local.addr),
                port: self.local.port,
            })
            .await
            .map_err(|e| TransportError::ConnectionRefused(format!("tcp accept failed: {e:?}")))?;
        let peer = socket.remote_endpoint().unwrap_or(self.local);
        debug!(peer = ?peer, "embedded tcp accepted connection");
        Ok(Some(TcpTransport::new(socket, peer)))
    }

    async fn close(&mut self) -> Result<()> {
        debug!(local = ?self.local, "embedded tcp listener closing");
        Ok(())
    }
}
