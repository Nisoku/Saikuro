use alloc::string::String;
use alloc::vec::Vec;

use embedded_io_async::{ErrorKind, Read, Write};
use wasi::io::streams::{InputStream, OutputStream};
use wasi::sockets::instance_network::instance_network;
use wasi::sockets::ip::{
    IpAddress, IpAddressFamily, IpSocketAddress, Ipv4Address, Ipv4SocketAddress, Ipv6Address,
    Ipv6SocketAddress,
};
use wasi::sockets::network::Network;
use wasi::sockets::tcp::{TcpSocket, TcpSocketType};

use crate::shared::error::{Result, TransportError};
use crate::wasi::tcp::{parse_addr};

/// A readable socket half.
pub struct Reader(InputStream);

/// A writable socket half.
pub struct Writer(OutputStream);

impl Read for Reader {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, ErrorKind> {
        self.0
            .blocking_read(buf)
            .map(|n| n as usize)
            .map_err(|_| ErrorKind::Other)
    }
}

impl Write for Writer {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, ErrorKind> {
        self.0
            .blocking_write(buf)
            .map(|_| buf.len())
            .map_err(|_| ErrorKind::Other)
    }

    async fn flush(&mut self) -> Result<(), ErrorKind> {
        self.0.blocking_flush().map_err(|_| ErrorKind::Other)
    }
}

/// A listening preview2 TCP socket.
pub struct Listener {
    socket: TcpSocket,
}

/// Map a preview2 socket error code into a transport error.
fn to_err(code: wasi::sockets::network::ErrorCode) -> TransportError {
    TransportError::ConnectionRefused(format!("{code:?}"))
}

/// Build an `IpSocketAddress` from a resolved IP and port.
fn socket_addr(ip: IpAddress, port: u16) -> IpSocketAddress {
    match ip {
        IpAddress::Ipv4(v4) => IpSocketAddress::Ipv4(Ipv4SocketAddress {
            port,
            address: v4,
        }),
        IpAddress::Ipv6(v6) => IpSocketAddress::Ipv6(Ipv6SocketAddress {
            port,
            address: v6,
            flow_info: 0,
            scope_id: 0,
        }),
    }
}

/// Dial `addr` (host:port) and return the connected read/write halves.
pub fn connect(addr: &str) -> Result<(Reader, Writer)> {
    let (host, port) = parse_addr(addr)?;
    let network = instance_network();
    let addrs = network
        .resolve_addresses(&host)
        .map_err(to_err)?;
    let ip = addrs
        .into_iter()
        .next()
        .ok_or_else(|| TransportError::ConnectionRefused(format!("no address for {host}")))?;
    let socket = TcpSocket::new(&network, IpAddressFamily::Ipv4, TcpSocketType::Stream)
        .map_err(to_err)?;
    socket
        .start_connect(&network, socket_addr(ip, port))
        .map_err(to_err)?;
    let (input, output) = socket.finish_connect().map_err(to_err)?;
    Ok((Reader(input), Writer(output)))
}

/// Bind and listen on `port` on all interfaces.
pub fn listen(port: u16) -> Result<Listener> {
    let network = instance_network();
    let socket = TcpSocket::new(&network, IpAddressFamily::Ipv4, TcpSocketType::Stream)
        .map_err(to_err)?;
    let local = IpSocketAddress::Ipv4(Ipv4SocketAddress {
        port,
        address: Ipv4Address {
            octets: [0, 0, 0, 0],
        },
    });
    socket.start_bind(&network, local).map_err(to_err)?;
    socket.finish_bind().map_err(to_err)?;
    socket.start_listen().map_err(to_err)?;
    socket.finish_listen().map_err(to_err)?;
    Ok(Listener { socket })
}

impl Listener {
    /// Accept one inbound connection and return its read/write halves.
    pub fn accept(&self) -> Result<(Reader, Writer)> {
        let (_new_socket, input, output) = self.socket.accept().map_err(to_err)?;
        Ok((Reader(input), Writer(output)))
    }
}
