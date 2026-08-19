use alloc::boxed::Box;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;
use async_trait::async_trait;
use bytes::Bytes;
#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic_util::Arc;

use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex as AsyncMutex;
use saikuro_event::{LogLevel, LogRecord};

use saikuro_net::net::tcp::TcpSocket;
use saikuro_net::net::{IpEndpoint, IpListenEndpoint, Stack};

use crate::shared::error::{Result, TransportError};
use crate::shared::framing::{read_frame, write_frame};
use crate::shared::traits::{
    Transport, TransportConnector, TransportListener, TransportReceiver, TransportSender,
};

const SOCKET_TX_SZ: usize = 1024;
const SOCKET_RX_SZ: usize = 1024;

// A listener accepts one connection at a time, so a single pair of static
// buffers is sufficient for both client and server sockets.  The socket borrows
// these for its lifetime, so they are kept `'static` and the transport types
// stay `Send`/`'static`.
static mut CLIENT_RX: [u8; SOCKET_RX_SZ] = [0; SOCKET_RX_SZ];
static mut CLIENT_TX: [u8; SOCKET_TX_SZ] = [0; SOCKET_TX_SZ];
static mut LISTENER_RX: [u8; SOCKET_RX_SZ] = [0; SOCKET_RX_SZ];
static mut LISTENER_TX: [u8; SOCKET_TX_SZ] = [0; SOCKET_TX_SZ];

type SharedSocket = Arc<AsyncMutex<NoopRawMutex, TcpSocket<'static>>>;

/// A TCP transport connection (embedded / embassy-net).
pub struct TcpTransport {
    socket: SharedSocket,
}

impl TcpTransport {
    /// Wrap an already-connected embassy-net [`TcpSocket`].
    pub fn new(socket: TcpSocket<'static>) -> Self {
        Self {
            socket: Arc::new(AsyncMutex::new(socket)),
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
            },
            TcpReceiver { socket },
        )
    }

    fn description(&self) -> &str {
        "tcp"
    }
}

/// Sending half of an embedded TCP transport.
pub struct TcpSender {
    socket: SharedSocket,
}

#[async_trait(?Send)]
impl TransportSender for TcpSender {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        let mut socket = self.socket.lock().await;
        write_frame(&mut *socket, &frame).await
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Receiving half of an embedded TCP transport.
pub struct TcpReceiver {
    socket: SharedSocket,
}

#[async_trait(?Send)]
impl TransportReceiver for TcpReceiver {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        let mut socket = self.socket.lock().await;
        read_frame(&mut *socket).await
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

#[async_trait(?Send)]
impl TransportConnector for TcpConnector {
    type Output = TcpTransport;

    async fn connect(&self) -> Result<Self::Output> {
        let mut record = LogRecord::now(
            LogLevel::Debug,
            "saikuro.transport.embedded.tcp",
            "embedded tcp connecting",
        );
        record.set_context("remote", alloc::format!("{:?}", self.remote));
        // NOTE: no log sink available in embedded connector; record is unused
        let rx = unsafe { &mut *core::ptr::addr_of_mut!(CLIENT_RX) };
        let tx = unsafe { &mut *core::ptr::addr_of_mut!(CLIENT_TX) };
        let mut socket = TcpSocket::new(*self.stack, rx, tx);
        socket
            .connect(self.remote)
            .await
            .map_err(|e| TransportError::ConnectionRefused(alloc::format!("{:?}", e)))?;
        Ok(TcpTransport::new(socket))
    }
}

/// Accepts incoming TCP connections over embassy-net.
///
/// embassy-net has no `TcpListener`: spin up a socket, put it in listening mode,
/// and await the single connection it accepts.
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

#[async_trait(?Send)]
impl TransportListener for TcpTransportListener {
    type Output = TcpTransport;

    async fn accept(&mut self) -> Result<Option<Self::Output>> {
        let rx = unsafe { &mut *core::ptr::addr_of_mut!(LISTENER_RX) };
        let tx = unsafe { &mut *core::ptr::addr_of_mut!(LISTENER_TX) };
        let mut socket = TcpSocket::new(*self.stack, rx, tx);
        socket
            .accept(IpListenEndpoint {
                addr: Some(self.local.addr),
                port: self.local.port,
            })
            .await
            .map_err(|e| TransportError::ConnectionRefused(alloc::format!("{:?}", e)))?;
        Ok(Some(TcpTransport::new(socket)))
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}
