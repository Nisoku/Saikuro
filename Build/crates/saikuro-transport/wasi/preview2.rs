use alloc::sync::Arc;

use wasi::io::streams::{InputStream, OutputStream};
use wasi::sockets::instance_network::instance_network;
use wasi::sockets::network::{
    ErrorCode, IpAddressFamily, IpSocketAddress, Ipv4SocketAddress, Network,
};
use wasi::sockets::tcp::TcpSocket;
use wasi::sockets::tcp_create_socket::create_tcp_socket;

use crate::shared::error::{Result, TransportError};
use crate::wasi::tcp::{parse_addr, parse_ipv4, WasiConn};

/// An open preview2 socket: holds the input/output streams.  Dropping the
/// streams closes the connection on the host (the generated resource handles
/// implement `Drop`).
pub struct Connection {
    input: InputStream,
    output: OutputStream,
}

/// A listening preview2 socket.
pub struct Listener {
    socket: TcpSocket,
}

/// Map a preview2 socket error code into a transport error.
fn to_err(code: ErrorCode) -> TransportError {
    TransportError::ConnectionRefused(format!("{code:?}"))
}

/// Build an `Ipv4SocketAddress` from a literal octet quad and port.  DNS is not
/// performed; only numeric IPv4 peers are supported, matching the preview1 path.
fn ipv4_socket_addr(octets: [u8; 4], port: u16) -> IpSocketAddress {
    IpSocketAddress::Ipv4(Ipv4SocketAddress {
        port,
        address: (octets[0], octets[1], octets[2], octets[3]),
    })
}

impl WasiConn for Connection {
    fn read_bytes(&self, buf: &mut [u8]) -> Result<usize> {
        let chunk = self
            .input
            .blocking_read(buf.len() as u64)
            .map_err(|e| TransportError::ReceiveFailed(format!("{e:?}")))?;
        let n = chunk.len().min(buf.len());
        buf[..n].copy_from_slice(&chunk[..n]);
        Ok(n)
    }

    fn write_bytes(&self, buf: &[u8]) -> Result<()> {
        self.output
            .blocking_write_and_flush(buf)
            .map_err(|e| TransportError::SendFailed(format!("{e:?}")))
    }
}

/// Send one length-prefixed frame over `conn`.
pub fn send_frame(conn: &Connection, frame: &[u8]) -> Result<()> {
    conn.output
        .blocking_write_and_flush(frame)
        .map_err(|e| TransportError::SendFailed(format!("{e:?}")))?;
    Ok(())
}

/// Dial `addr` (host:port) and return the connected socket.  `host` must be a
/// numeric IPv4 literal (no DNS resolution on the preview2 path).
pub fn connect(addr: &str) -> Result<Arc<Connection>> {
    let (host, port) = parse_addr(addr)?;
    let octets = parse_ipv4(&host)
        .ok_or_else(|| TransportError::ConnectionRefused(format!("unresolved host {host}")))?;
    let network: Network = instance_network();
    let socket = create_tcp_socket(IpAddressFamily::Ipv4).map_err(to_err)?;
    socket
        .start_connect(&network, ipv4_socket_addr(octets, port))
        .map_err(to_err)?;
    let (input, output) = socket.finish_connect().map_err(to_err)?;
    Ok(Arc::new(Connection { input, output }))
}

/// Bind and listen on `port` on all interfaces.
pub fn listen(port: u16) -> Result<Listener> {
    let network: Network = instance_network();
    let socket = create_tcp_socket(IpAddressFamily::Ipv4).map_err(to_err)?;
    socket
        .start_bind(
            &network,
            IpSocketAddress::Ipv4(Ipv4SocketAddress {
                port,
                address: (0, 0, 0, 0),
            }),
        )
        .map_err(to_err)?;
    socket.finish_bind().map_err(to_err)?;
    socket.start_listen().map_err(to_err)?;
    socket.finish_listen().map_err(to_err)?;
    Ok(Listener { socket })
}

impl Listener {
    /// Accept one inbound connection and return its socket.
    pub fn accept(&self) -> Result<Arc<Connection>> {
        let (_new_socket, input, output) = self.socket.accept().map_err(to_err)?;
        Ok(Arc::new(Connection { input, output }))
    }
}
