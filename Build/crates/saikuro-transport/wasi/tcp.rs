use alloc::boxed::Box;
use alloc::string::{String, ToString};
#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic_util::Arc;
#[cfg(target_has_atomic = "ptr")]
use saikuro_core::Arc;

use async_trait::async_trait;
use bytes::Bytes;

use crate::shared::error::{Result, TransportError};
use crate::shared::framing::{read_frame, write_frame, AsyncByteRead, AsyncByteWrite};
use crate::shared::traits::{
    LocalTransport, LocalTransportConnector, LocalTransportListener, LocalTransportReceiver,
    LocalTransportSender,
};
use crate::wasi::tcp::backend::{Connection, Listener};

#[cfg(all(feature = "wasi-preview2", feature = "wasi-preview1"))]
compile_error!(
    "saikuro-transport: enable exactly one of wasi-preview1 / wasi-preview2 for wasi-tcp/wasi-host"
);

#[cfg(not(any(feature = "wasi-preview2", feature = "wasi-preview1")))]
compile_error!(
    "saikuro-transport: enable wasi-preview1 or wasi-preview2 to select the WASI socket backend"
);

#[cfg(feature = "wasi-preview1")]
pub use crate::wasi::preview1 as backend;
#[cfg(feature = "wasi-preview2")]
pub use crate::wasi::preview2 as backend;

/// Raw byte I/O over a WASI socket connection.
pub trait WasiConn {
    /// Read up to `buf.len()` bytes into `buf`, returning the count (0 = EOF).
    fn read_bytes(&self, buf: &mut [u8]) -> Result<usize>;
    /// Write the entirety of `buf`.
    fn write_bytes(&self, buf: &[u8]) -> Result<()>;
}

/// Borrowing reader half that adapts a [`WasiConn`] to [`AsyncByteRead`].
pub struct WasiReader<'a, C: WasiConn>(&'a C);

/// Borrowing writer half that adapts a [`WasiConn`] to [`AsyncByteWrite`].
pub struct WasiWriter<'a, C: WasiConn>(&'a C);

impl<'a, C: WasiConn> AsyncByteRead for WasiReader<'a, C> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.read_bytes(buf)
    }
}

impl<'a, C: WasiConn> AsyncByteWrite for WasiWriter<'a, C> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.write_bytes(buf)?;
        Ok(buf.len())
    }

    async fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

/// A length-prefixed WASI TCP transport.  Both halves share one socket.
pub struct WasiTcpTransport {
    conn: Arc<Connection>,
    peer: String,
}

impl WasiTcpTransport {
    /// Wrap an already-connected socket.
    pub fn new(conn: Arc<Connection>, peer: String) -> Self {
        Self { conn, peer }
    }

    /// Return the address this transport is connected to.
    pub fn peer_addr(&self) -> &str {
        &self.peer
    }
}

impl LocalTransport for WasiTcpTransport {
    type Sender = WasiTcpSender;
    type Receiver = WasiTcpReceiver;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        (
            WasiTcpSender {
                conn: self.conn.clone(),
            },
            WasiTcpReceiver { conn: self.conn },
        )
    }

    fn description(&self) -> &str {
        "wasi-tcp"
    }
}

/// Sending half of a [`WasiTcpTransport`].
pub struct WasiTcpSender {
    conn: Arc<Connection>,
}

#[async_trait(?Send)]
impl LocalTransportSender for WasiTcpSender {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        write_frame(&mut WasiWriter(self.conn.as_ref()), &frame).await
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Receiving half of a [`WasiTcpTransport`].
pub struct WasiTcpReceiver {
    conn: Arc<Connection>,
}

#[async_trait(?Send)]
impl LocalTransportReceiver for WasiTcpReceiver {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        read_frame(&mut WasiReader(self.conn.as_ref())).await
    }
}

/// Connects to a peer over WASI TCP.
pub struct WasiTcpConnector {
    addr: String,
}

impl WasiTcpConnector {
    /// Create a connector that will dial `addr` (host:port).
    pub fn new(addr: impl Into<String>) -> Self {
        Self { addr: addr.into() }
    }
}

#[async_trait(?Send)]
impl LocalTransportConnector for WasiTcpConnector {
    type Output = WasiTcpTransport;

    async fn connect(&self) -> Result<Self::Output> {
        let conn = backend::connect(&self.addr)?;
        Ok(WasiTcpTransport::new(conn, self.addr.clone()))
    }
}

/// Accepts inbound WASI TCP connections on a port.
pub struct WasiTcpListener {
    inner: Listener,
}

impl WasiTcpListener {
    /// Start listening on `addr` (host:port); only the port is used.
    pub fn new(addr: impl Into<String>) -> Result<Self> {
        let (_, port) = parse_addr(&addr.into())?;
        Ok(Self {
            inner: backend::listen(port)?,
        })
    }
}

#[async_trait(?Send)]
impl LocalTransportListener for WasiTcpListener {
    type Output = WasiTcpTransport;

    async fn accept(&mut self) -> Result<Option<Self::Output>> {
        let conn = self.inner.accept()?;
        Ok(Some(WasiTcpTransport::new(conn, String::new())))
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Split `host:port` into its pieces.
pub fn parse_addr(addr: &str) -> Result<(String, u16)> {
    let (host, port_str) = addr
        .rsplit_once(':')
        .ok_or_else(|| TransportError::ConnectionRefused(format!("missing port in {addr}")))?;
    let port = port_str
        .parse::<u16>()
        .map_err(|_| TransportError::ConnectionRefused(format!("bad port in {addr}")))?;
    Ok((host.to_string(), port))
}

/// Parse a dotted-quad IPv4 literal.
pub fn parse_ipv4(host: &str) -> Option<[u8; 4]> {
    let mut it = host.split('.');
    let a: u8 = it.next()?.parse().ok()?;
    let b: u8 = it.next()?.parse().ok()?;
    let c: u8 = it.next()?.parse().ok()?;
    let d: u8 = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some([a, b, c, d])
}
