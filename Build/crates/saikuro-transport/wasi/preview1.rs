use alloc::sync::Arc;

use crate::shared::error::{Result, TransportError};
use crate::wasi::tcp::{parse_addr, parse_ipv4, WasiConn};

const AF_INET: u8 = 0; // witx address-family::inet4
const SOCK_STREAM: u8 = 1; // witx socket-type::stream

#[repr(C)]
struct Ciovec {
    buf: *const u8,
    len: usize,
}

#[repr(C)]
struct Iovec {
    buf: *const u8,
    len: usize,
}

#[repr(C)]
struct SockaddrIn {
    sin_family: u8,
    sin_port: u16,
    sin_addr: u32,
    sin_zero: [u8; 8],
}

#[repr(C)]
struct RecvRet {
    len: u32,
    roflags: u16,
}

#[link(wasm_import_module = "wasi_snapshot_preview1")]
extern "C" {
    fn sock_open(family: u8, ty: u8, ret_area: *mut u32) -> u16;
    fn sock_connect(fd: u32, addr: *const SockaddrIn, addr_len: u32) -> u16;
    fn sock_bind(fd: u32, addr: *const SockaddrIn, addr_len: u32) -> u16;
    fn sock_listen(fd: u32, backlog: u32) -> u16;
    fn sock_accept(fd: u32, flags: *mut u16, ret_area: *mut u32) -> u16;
    fn sock_recv(fd: u32, ri_data: *const Ciovec, ri_flags: u16, ret_area: *mut RecvRet) -> u16;
    fn sock_send(fd: u32, si_data: *const Iovec, si_flags: u16, ret_area: *mut u32) -> u16;
    fn fd_close(fd: u32) -> u16;
}

/// An open preview1 socket.  Owns the fd: the last `Arc` dropping closes it.
pub struct Connection {
    fd: u32,
}

impl Drop for Connection {
    fn drop(&mut self) {
        // SAFETY: fd is a valid open socket; fd_close frees it on the host.
        unsafe {
            let _ = fd_close(self.fd);
        }
    }
}

/// A listening preview1 socket.
pub struct Listener {
    fd: u32,
}

impl Drop for Listener {
    fn drop(&mut self) {
        // SAFETY: fd is a valid open listening socket; fd_close frees it.
        unsafe {
            let _ = fd_close(self.fd);
        }
    }
}

fn errno_ok(code: u16) -> bool {
    code == 0
}

fn sockaddr_in(octets: [u8; 4], port: u16) -> SockaddrIn {
    SockaddrIn {
        sin_family: AF_INET,
        sin_port: port.to_be(),
        // The socket layer reads sin_addr as raw network-order bytes.  A u32
        // stored little-endian has those same bytes in memory order [a,b,c,d],
        // which is exactly what the host expects, so load the octets LE.
        sin_addr: u32::from_le_bytes(octets),
        sin_zero: [0; 8],
    }
}

/// Receive up to `buf.len()` bytes into `buf`; returns the count read.
/// A return of `0` indicates a clean EOF.
fn recv_raw(fd: u32, buf: &mut [u8]) -> Result<usize> {
    let iov = Ciovec {
        buf: buf.as_ptr(),
        len: buf.len(),
    };
    let mut ret = RecvRet { len: 0, roflags: 0 };
    // SAFETY: iov aliases buf for the duration of the call and ret is written
    // by the host. The fd is a valid open socket.
    let rc = unsafe { sock_recv(fd, &iov, 0, &mut ret) };
    if !errno_ok(rc) {
        return Err(TransportError::ReceiveFailed(format!("sock_recv: {rc}")));
    }
    Ok(ret.len as usize)
}

impl WasiConn for Connection {
    fn read_bytes(&self, buf: &mut [u8]) -> Result<usize> {
        recv_raw(self.fd, buf)
    }

    fn write_bytes(&self, buf: &[u8]) -> Result<()> {
        send_frame(self, buf)
    }
}

/// Send one length-prefixed frame over `conn`.
pub fn send_frame(conn: &Connection, frame: &[u8]) -> Result<()> {
    let mut offset = 0;
    while offset < frame.len() {
        let iov = Iovec {
            buf: frame[offset..].as_ptr(),
            len: frame.len() - offset,
        };
        let mut n = 0u32;
        // SAFETY: iov aliases frame for the duration of the call; n is written
        // by the host. The fd is a valid open socket.
        let rc = unsafe { sock_send(conn.fd, &iov, 0, &mut n) };
        if !errno_ok(rc) {
            return Err(TransportError::SendFailed(format!("sock_send: {rc}")));
        }
        if n == 0 {
            return Err(TransportError::SendFailed("sock_send wrote 0 bytes".into()));
        }
        offset += n as usize;
    }
    Ok(())
}

/// Dial `addr` (host:port) and return the connected socket.
pub fn connect(addr: &str) -> Result<Arc<Connection>> {
    let (host, port) = parse_addr(addr)?;
    let octets = parse_ipv4(&host)
        .ok_or_else(|| TransportError::ConnectionRefused(format!("unresolved host {host}")))?;

    let mut fd = 0u32;
    // SAFETY: sock_open writes exactly one fd to ret_area on success.
    let rc = unsafe { sock_open(AF_INET, SOCK_STREAM, &mut fd) };
    if !errno_ok(rc) {
        return Err(TransportError::ConnectionRefused(format!(
            "sock_open: {rc}"
        )));
    }
    let conn = Arc::new(Connection { fd });
    let sa = sockaddr_in(octets, port);
    // SAFETY: sa points to a valid SockaddrIn for the duration of the call.
    let rc = unsafe { sock_connect(conn.fd, &sa, core::mem::size_of::<SockaddrIn>() as u32) };
    if !errno_ok(rc) {
        return Err(TransportError::ConnectionRefused(format!(
            "sock_connect: {rc}"
        )));
    }
    Ok(conn)
}

/// Bind and listen on `port` on all interfaces.
pub fn listen(port: u16) -> Result<Listener> {
    let mut fd = 0u32;
    let rc = unsafe { sock_open(AF_INET, SOCK_STREAM, &mut fd) };
    if !errno_ok(rc) {
        return Err(TransportError::ConnectionRefused(format!(
            "sock_open: {rc}"
        )));
    }
    let sa = sockaddr_in([0, 0, 0, 0], port);
    let rc = unsafe { sock_bind(fd, &sa, core::mem::size_of::<SockaddrIn>() as u32) };
    if !errno_ok(rc) {
        // SAFETY: fd is a valid open socket; free it before reporting failure.
        unsafe {
            let _ = fd_close(fd);
        }
        return Err(TransportError::ConnectionRefused(format!(
            "sock_bind: {rc}"
        )));
    }
    let rc = unsafe { sock_listen(fd, 16) };
    if !errno_ok(rc) {
        // SAFETY: fd is a valid open socket; free it before reporting failure.
        unsafe {
            let _ = fd_close(fd);
        }
        return Err(TransportError::ConnectionRefused(format!(
            "sock_listen: {rc}"
        )));
    }
    Ok(Listener { fd })
}

impl Listener {
    /// Accept one inbound connection and return its socket.
    pub fn accept(&self) -> Result<Arc<Connection>> {
        let mut flags = 0u16;
        let mut fd = 0u32;
        // SAFETY: host writes the accepted fd to ret_area; flags is read by host.
        let rc = unsafe { sock_accept(self.fd, &mut flags, &mut fd) };
        if !errno_ok(rc) {
            return Err(TransportError::ConnectionRefused(format!(
                "sock_accept: {rc}"
            )));
        }
        Ok(Arc::new(Connection { fd }))
    }
}
