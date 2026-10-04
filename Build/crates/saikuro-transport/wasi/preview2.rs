#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic_util::Arc;
#[cfg(target_has_atomic = "ptr")]
use saikuro_core::Arc;

use core::future::poll_fn;
use core::task::Poll;

use wasi::io::poll::Pollable;
use wasi::io::streams::{InputStream, OutputStream, StreamError};
use wasi::sockets::instance_network::instance_network;
use wasi::sockets::network::{
    ErrorCode, IpAddressFamily, IpSocketAddress, Ipv4SocketAddress, Network,
};
use wasi::sockets::tcp::TcpSocket;
use wasi::sockets::tcp_create_socket::create_tcp_socket;

use crate::shared::error::{Result, TransportError};
use crate::wasi::tcp::{parse_addr, parse_ipv4, WasiAsyncConn, WasiConn};

/// An open preview2 socket: holds the input/output streams.  Dropping the
/// streams closes the connection on the host (the generated resource handles
/// implement `Drop`).
pub struct Connection {
    input: InputStream,
    output: OutputStream,
    _socket: TcpSocket,
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

/// Yield until `pollable` reports ready.
async fn wait_ready(pollable: &Pollable) {
    poll_fn(|cx| {
        if pollable.ready() {
            Poll::Ready(())
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
    .await
}

impl WasiAsyncConn for Connection {
    async fn read_bytes(&self, buf: &mut [u8]) -> Result<usize> {
        loop {
            match self.input.read(buf.len() as u64) {
                Ok(chunk) => {
                    let n = chunk.len().min(buf.len());
                    if n == 0 {
                        // No bytes buffered yet
                        wait_ready(&self.input.subscribe()).await;
                        continue;
                    }
                    buf[..n].copy_from_slice(&chunk[..n]);
                    return Ok(n);
                }
                Err(StreamError::Closed) => return Ok(0),
                Err(e) => return Err(TransportError::ReceiveFailed(format!("{e:?}"))),
            }
        }
    }

    async fn write_bytes(&self, buf: &[u8]) -> Result<()> {
        let mut written = 0;
        while written < buf.len() {
            let permit = loop {
                match self.output.check_write() {
                    Ok(0) => wait_ready(&self.output.subscribe()).await,
                    Ok(n) => break n,
                    Err(StreamError::Closed) => {
                        return Err(TransportError::SendFailed("stream closed".into()));
                    }
                    Err(e) => return Err(TransportError::SendFailed(format!("{e:?}"))),
                }
            };
            // A `write` longer than the permit traps
            let permit = usize::try_from(permit)
                .map_err(|_| TransportError::SendFailed("write permit overflow".into()))?;
            let n = permit.min(buf.len() - written);
            if n == 0 {
                wait_ready(&self.output.subscribe()).await;
                continue;
            }
            self.output
                .write(&buf[written..written + n])
                .map_err(|e| TransportError::SendFailed(format!("{e:?}")))?;
            written += n;
        }
        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        self.output
            .flush()
            .map_err(|e| TransportError::SendFailed(format!("{e:?}")))
    }
}

/// Dial `addr` (host:port) and return the connected socket.  `host` must be a
/// numeric IPv4 literal (no DNS resolution on the preview2 path).
pub async fn connect(addr: &str) -> Result<Arc<Connection>> {
    let (host, port) = parse_addr(addr)?;
    let octets = parse_ipv4(&host)
        .ok_or_else(|| TransportError::ConnectionRefused(format!("unresolved host {host}")))?;
    let network: Network = instance_network();
    let socket = create_tcp_socket(IpAddressFamily::Ipv4).map_err(to_err)?;
    socket
        .start_connect(&network, ipv4_socket_addr(octets, port))
        .map_err(to_err)?;
    // Preview2 sockets are non-blocking: the handshake completes on the host
    // while this task yields, letting the executor run other tasks meanwhile.
    let ready = socket.subscribe();
    let (input, output) = loop {
        match socket.finish_connect() {
            Ok(connected) => break connected,
            Err(ErrorCode::WouldBlock) => wait_ready(&ready).await,
            Err(e) => return Err(to_err(e)),
        }
    };
    Ok(Arc::new(Connection {
        input,
        output,
        _socket: socket,
    }))
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
    /// Return the local port this listener is bound to.
    pub fn local_port(&self) -> Result<u16> {
        match self.socket.local_address().map_err(to_err)? {
            IpSocketAddress::Ipv4(addr) => Ok(addr.port),
            IpSocketAddress::Ipv6(addr) => Ok(addr.port),
        }
    }

    /// Accept one inbound connection and return its socket.
    pub async fn accept(&self) -> Result<Arc<Connection>> {
        let ready = self.socket.subscribe();
        let (socket, input, output) = loop {
            match self.socket.accept() {
                Ok(connection) => break connection,
                Err(ErrorCode::WouldBlock) => wait_ready(&ready).await,
                Err(e) => return Err(to_err(e)),
            }
        };
        Ok(Arc::new(Connection {
            input,
            output,
            _socket: socket,
        }))
    }
}
