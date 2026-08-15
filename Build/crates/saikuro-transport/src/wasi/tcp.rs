use alloc::string::{String, ToString};
use alloc::vec::Vec;

use bytes::Bytes;
use embedded_io_async::{Read, Write};

use crate::shared::error::{Result, TransportError};
use crate::shared::framed::{read_exact, read_first_byte, write_all};
use crate::shared::traits::{
    LocalTransport, LocalTransportConnector, LocalTransportListener, LocalTransportReceiver,
    LocalTransportSender,
};

#[cfg(all(feature = "wasi-preview2", feature = "wasi-preview1"))]
compile_error!(
    "saikuro-transport: enable exactly one of wasi-preview1 / wasi-preview2 for wasi-tcp/wasi-host"
);

#[cfg(not(any(feature = "wasi-preview2", feature = "wasi-preview1")))]
compile_error!(
    "saikuro-transport: enable wasi-preview1 or wasi-preview2 to select the WASI socket backend"
);

#[cfg(feature = "wasi-preview2")]
pub use preview2 as backend;
#[cfg(feature = "wasi-preview1")]
pub use preview1 as backend;

/// A length-prefixed WASI TCP transport.
pub struct WasiTcpTransport<R, W> {
    reader: R,
    writer: W,
    peer: String,
}

impl<R: Read + Unpin, W: Write + Unpin> WasiTcpTransport<R, W> {
    /// Wrap an already-connected `(reader, writer)` pair.
    pub fn new(reader: R, writer: W, peer: String) -> Self {
        Self { reader, writer, peer }
    }
}

impl<R: Read + Unpin, W: Write + Unpin> LocalTransport for WasiTcpTransport<R, W> {
    type Sender = WasiTcpSender<W>;
    type Receiver = WasiTcpReceiver<R>;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        (
            WasiTcpSender {
                writer: self.writer,
            },
            WasiTcpReceiver {
                reader: self.reader,
            },
        )
    }

    fn description(&self) -> &str {
        "wasi-tcp"
    }
}

/// Sending half of a [`WasiTcpTransport`].
pub struct WasiTcpSender<W> {
    writer: W,
}

impl<W: Write + Unpin> LocalTransportSender for WasiTcpSender<W> {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        write_a_frame(&mut self.writer, &frame).await
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Receiving half of a [`WasiTcpTransport`].
pub struct WasiTcpReceiver<R> {
    reader: R,
}

impl<R: Read + Unpin> LocalTransportReceiver for WasiTcpReceiver<R> {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        read_a_frame(&mut self.reader).await
    }
}

/// Write one length-prefixed frame to `writer`.
async fn write_a_frame<W: Write + Unpin>(writer: &mut W, frame: &[u8]) -> Result<()> {
    let len = frame.len();
    let header = [
        (len >> 24) as u8,
        (len >> 16) as u8,
        (len >> 8) as u8,
        len as u8,
    ];
    write_all(writer, &header).await?;
    write_all(writer, frame).await?;
    Ok(())
}

/// Read one length-prefixed frame, or `None` on a clean zero-length close.
async fn read_a_frame<R: Read + Unpin>(reader: &mut R) -> Result<Option<Bytes>> {
    let mut first = 0u8;
    read_first_byte(reader, &mut first).await?;
    if first == 0 {
        return Ok(None);
    }
    let mut rest = [0u8; 3];
    read_exact(reader, &mut rest, "wasi-tcp: closed during frame header").await?;
    let len = ((first as usize) << 24)
        | ((rest[0] as usize) << 16)
        | ((rest[1] as usize) << 8)
        | (rest[2] as usize);
    let mut buf = Vec::with_capacity(len);
    buf.resize(len, 0);
    read_exact(reader, &mut buf, "wasi-tcp: closed during frame payload").await?;
    Ok(Some(Bytes::from(buf)))
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

impl LocalTransportConnector for WasiTcpConnector {
    type Output = WasiTcpTransport<backend::Reader, backend::Writer>;

    async fn connect(&self) -> Result<Self::Output> {
        let (reader, writer) = backend::connect(&self.addr)?;
        Ok(WasiTcpTransport::new(reader, writer, self.addr.clone()))
    }
}

/// Accepts inbound WASI TCP connections on a port.
pub struct WasiTcpListener {
    inner: backend::Listener,
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

impl LocalTransportListener for WasiTcpListener {
    type Output = WasiTcpTransport<backend::Reader, backend::Writer>;

    async fn accept(&mut self) -> Result<Option<Self::Output>> {
        let (reader, writer) = self.inner.accept()?;
        Ok(Some(WasiTcpTransport::new(reader, writer, String::new())))
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
