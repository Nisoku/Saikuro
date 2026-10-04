use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic_util::Arc;
#[cfg(target_has_atomic = "ptr")]
use saikuro_core::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use core::cell::RefCell;

use crate::shared::error::{Result, TransportError};
#[cfg(not(feature = "native"))]
use crate::shared::traits::{
    LocalTransport, LocalTransportListener, LocalTransportReceiver, LocalTransportSender,
};
use crate::shared::traits::{TransportReceiver, TransportSender};
use crate::wasi::tcp::backend::Connection;
use crate::wasi::tcp::{WasiAsyncConn, WasiTcpListener};

use embedded_websocket::{
    read_http_header, Error as WsError, WebSocket, WebSocketClient, WebSocketCloseStatusCode,
    WebSocketOptions, WebSocketReceiveMessageType, WebSocketSendMessageType, WebSocketServer,
    WebSocketType,
};
use rand_core_06::RngCore;

/// Socket read scratch and frame payload buffer size.
const WS_BUF: usize = 4096;
/// Largest frame header: 2 base bytes + 8 extended length + 4 mask key.
const WS_HEADER_RESERVE: usize = 14;
/// Outbound payload chunk that still leaves room for the frame header and mask.
const WS_PAYLOAD: usize = WS_BUF - WS_HEADER_RESERVE;
/// Safety cap on unparsed inbound bytes retained between socket reads.
const WS_MAX_BUFFER: usize = 64 * 1024;
/// Safety cap on the HTTP handshake bytes accumulated in either direction.
const WS_HANDSHAKE_MAX: usize = 16 * 1024;
/// Buffer for a control frame: 125-byte payload limit plus the 6-byte masked header.
const WS_CTRL: usize = 132;
/// Maximum number of HTTP request headers accepted during server handshake.
const WS_MAX_HEADERS: usize = 16;
/// Upper bound on the HTTP response written during the server handshake.
const WS_RESPONSE_MAX: usize = 512;

/// Mask-key entropy for the client role.
pub struct WsRng;

impl RngCore for WsRng {
    fn next_u32(&mut self) -> u32 {
        let mut b = [0u8; 4];
        self.fill_bytes(&mut b);
        u32::from_le_bytes(b)
    }

    fn next_u64(&mut self) -> u64 {
        let mut b = [0u8; 8];
        self.fill_bytes(&mut b);
        u64::from_le_bytes(b)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        getrandom::fill(dest).expect("ws rng: getrandom failed on WASI")
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> core::result::Result<(), rand_core_06::Error> {
        match getrandom::fill(dest) {
            Ok(()) => Ok(()),
            Err(_) => Err(rand_core_06::Error::from(
                core::num::NonZeroU32::new(0x10000u32).expect("nonzero"),
            )),
        }
    }
}

/// Parse `ws://host[:port][/path]`.  WASI exposes only raw TCP, so TLS-based
/// `wss://` is not supported here.
fn parse_ws_url(url: &str) -> Result<(String, u16, String)> {
    let rest = url.strip_prefix("ws://").ok_or_else(|| {
        TransportError::ConnectionRefused(format!(
            "ws: only non-TLS ws:// is supported on WASI: {url}"
        ))
    })?;
    let (authority, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], format!("/{}", &rest[idx + 1..])),
        None => (rest, String::from("/")),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (
            String::from(h),
            p.parse::<u16>()
                .map_err(|_| TransportError::ConnectionRefused(format!("ws: bad port in {url}")))?,
        ),
        None => (String::from(authority), 80),
    };
    Ok((host, port, path))
}

/// Map a sans-io WebSocket error into a transport error.
fn map_ws_err(e: WsError) -> TransportError {
    match e {
        WsError::WebSocketNotOpen => TransportError::ConnectionLost("ws: socket not open".into()),
        WsError::WriteToBufferTooSmall => TransportError::MessageTooLarge {
            size: WS_BUF,
            limit: WS_BUF,
        },
        WsError::ReadFrameIncomplete => {
            TransportError::ReceiveFailed("ws: incomplete frame header".into())
        }
        WsError::HttpHeaderIncomplete => {
            TransportError::ConnectionRefused("ws: incomplete handshake response".into())
        }
        other => TransportError::ReceiveFailed(format!("ws: {other:?}")),
    }
}

/// Locate the `\r\n\r\n` that terminates an HTTP header block, returning the
/// block length including the terminator.
fn header_block_len(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|idx| idx + 4)
}

/// Split an HTTP request into `(name, value)` header pairs.
///
/// `buf` must begin at the request line and end at the blank line that closes
/// the header block.  Returns `None` when a line carries no `:` separator, a
/// name is not valid UTF-8, or the header count exceeds [`WS_MAX_HEADERS`].
///
/// See: RFC 7230 section 3.2.
fn split_request_headers(buf: &[u8]) -> Option<Vec<(&str, &[u8])>> {
    let block_len = header_block_len(buf)?;
    let mut lines = buf[..block_len - 4]
        .split(|&byte| byte == b'\n')
        .map(|line| line.strip_suffix(b"\r").unwrap_or(line));
    let _request_line = lines.next()?;
    let mut headers = Vec::new();
    for line in lines {
        let colon = line.iter().position(|&byte| byte == b':')?;
        let name = core::str::from_utf8(&line[..colon]).ok()?;
        let raw = &line[colon + 1..];
        let value = raw.strip_prefix(b" ").unwrap_or(raw);
        headers.push((name, value));
        if headers.len() > WS_MAX_HEADERS {
            return None;
        }
    }
    Some(headers)
}

/// Shared per-connection state
struct WasiWsState<W> {
    ws: W,
    /// Bytes read from the socket but not yet consumed by the frame parser.
    rx: Vec<u8>,
    /// Payload accumulated for the message currently being decoded.
    frame: Vec<u8>,
    /// Set once the peer closed the connection or a close was acknowledged.
    closed: bool,
}

/// Perform the client opening handshake over `conn`.
async fn client_handshake(
    conn: &Connection,
    ws: &mut WebSocketClient<WsRng>,
    options: &WebSocketOptions<'_>,
) -> Result<Vec<u8>> {
    let mut request = [0u8; 1024];
    let (len, key) = ws
        .client_connect(options, &mut request)
        .map_err(|e| TransportError::ConnectionRefused(format!("ws: build handshake: {e:?}")))?;
    conn.write_bytes(&request[..len]).await?;
    conn.flush().await?;

    let mut response: Vec<u8> = Vec::new();
    let mut buf = [0u8; 512];
    loop {
        match ws.client_accept(&key, &response) {
            Ok((header_len, _sub_protocol)) => return Ok(response.split_off(header_len)),
            Err(WsError::HttpHeaderIncomplete) => {}
            Err(e) => {
                return Err(TransportError::ConnectionRefused(format!(
                    "ws: handshake response: {e:?}"
                )));
            }
        }
        let n = conn.read_bytes(&mut buf).await?;
        if n == 0 {
            return Err(TransportError::ConnectionRefused(
                "ws: peer closed during handshake".into(),
            ));
        }
        response.extend_from_slice(&buf[..n]);
        if response.len() > WS_HANDSHAKE_MAX {
            return Err(TransportError::ConnectionRefused(
                "ws: handshake response too large".into(),
            ));
        }
    }
}

/// Perform the server opening handshake over `conn`.
async fn server_handshake(conn: &Connection, ws: &mut WebSocketServer) -> Result<Vec<u8>> {
    let mut request: Vec<u8> = Vec::new();
    let mut buf = [0u8; 512];
    let header_end = loop {
        if let Some(end) = header_block_len(&request) {
            break end;
        }
        let n = conn.read_bytes(&mut buf).await?;
        if n == 0 {
            return Err(TransportError::ConnectionRefused(
                "ws: peer closed during handshake".into(),
            ));
        }
        request.extend_from_slice(&buf[..n]);
        if request.len() > WS_HANDSHAKE_MAX {
            return Err(TransportError::ConnectionRefused(
                "ws: handshake request too large".into(),
            ));
        }
    };

    let headers = split_request_headers(&request[..header_end])
        .ok_or_else(|| TransportError::ConnectionRefused("ws: malformed upgrade request".into()))?;
    let context = read_http_header(headers.into_iter())
        .map_err(|e| TransportError::ConnectionRefused(format!("ws: upgrade request: {e:?}")))?
        .ok_or_else(|| {
            TransportError::ConnectionRefused("ws: not a WebSocket upgrade request".into())
        })?;

    let mut response = [0u8; WS_RESPONSE_MAX];
    let len = ws
        .server_accept(&context.sec_websocket_key, None, &mut response)
        .map_err(map_ws_err)?;
    conn.write_bytes(&response[..len]).await?;
    conn.flush().await?;

    Ok(request.split_off(header_end))
}

/// A `no_std` WebSocket transport backed by a WASI socket, generic over the
/// `embedded-websocket` role.
pub struct WsConn<W> {
    state: Arc<RefCell<WasiWsState<W>>>,
    conn: Arc<Connection>,
}

/// A WebSocket client transport, dialed out to a remote provider.
pub type WebSocketTransport = WsConn<WebSocketClient<WsRng>>;
/// A WebSocket server transport, adopted from an accepted socket.
pub type WebSocketServerTransport = WsConn<WebSocketServer>;

#[allow(clippy::arc_with_non_send_sync)]
impl WebSocketTransport {
    /// Open a WebSocket connection to `url` (`ws://host[:port][/path]`).
    pub async fn connect(url: impl Into<String>) -> Result<Self> {
        let url = url.into();
        let (host, port, path) = parse_ws_url(&url)?;
        let conn = crate::wasi::tcp::backend::connect(&format!("{host}:{port}")).await?;
        let mut ws = WebSocketClient::new_client(WsRng);
        let options = WebSocketOptions {
            path: &path,
            host: &host,
            origin: &host,
            sub_protocols: None,
            additional_headers: None,
        };
        let rx = client_handshake(&conn, &mut ws, &options).await?;
        Ok(Self {
            state: Arc::new(RefCell::new(WasiWsState {
                ws,
                rx,
                frame: Vec::new(),
                closed: false,
            })),
            conn,
        })
    }
}

impl<R, T> LocalTransport for WsConn<WebSocket<R, T>>
where
    R: RngCore + 'static,
    T: WebSocketType + 'static,
{
    type Sender = WsSender<WebSocket<R, T>>;
    type Receiver = WsRecv<WebSocket<R, T>>;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        (
            WsSender {
                state: self.state.clone(),
                conn: self.conn.clone(),
            },
            WsRecv {
                state: self.state,
                conn: self.conn,
            },
        )
    }

    fn description(&self) -> &str {
        "websocket"
    }
}

#[cfg(not(feature = "native"))]
#[async_trait(?Send)]
impl<R, T> LocalTransportSender for WsSender<WebSocket<R, T>>
where
    R: RngCore + 'static,
    T: WebSocketType + 'static,
{
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        TransportSender::send(self, frame).await
    }

    async fn close(&mut self) -> Result<()> {
        TransportSender::close(self).await
    }
}

#[cfg(not(feature = "native"))]
#[async_trait(?Send)]
impl<R, T> LocalTransportReceiver for WsRecv<WebSocket<R, T>>
where
    R: RngCore + 'static,
    T: WebSocketType + 'static,
{
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        TransportReceiver::recv(self).await
    }
}

/// Sending half of a [`WebSocketTransport`] or [`WebSocketServerTransport`].
pub struct WsSender<W> {
    state: Arc<RefCell<WasiWsState<W>>>,
    conn: Arc<Connection>,
}

/// Sending half of a client-role [`WebSocketTransport`].
pub type WebSocketSender = WsSender<WebSocketClient<WsRng>>;
/// Sending half of a server-role [`WebSocketServerTransport`].
pub type WebSocketServerSender = WsSender<WebSocketServer>;

#[async_trait(?Send)]
impl<R, T> TransportSender for WsSender<WebSocket<R, T>>
where
    R: RngCore + 'static,
    T: WebSocketType + 'static,
{
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        let mut out = [0u8; WS_BUF];
        let mut remaining = frame.as_ref();
        loop {
            let chunk_len = remaining.len().min(WS_PAYLOAD);
            let end_of_message = remaining.len() <= WS_PAYLOAD;
            let len = {
                let mut st = self.state.borrow_mut();
                if st.closed {
                    return Err(TransportError::ConnectionLost(
                        "ws: send after close".into(),
                    ));
                }
                st.ws
                    .write(
                        WebSocketSendMessageType::Binary,
                        end_of_message,
                        &remaining[..chunk_len],
                        &mut out,
                    )
                    .map_err(map_ws_err)?
            };
            self.conn.write_bytes(&out[..len]).await?;
            remaining = &remaining[chunk_len..];
            if end_of_message {
                break;
            }
        }
        self.conn.flush().await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        let mut out = [0u8; WS_BUF];
        let len = {
            let mut st = self.state.borrow_mut();
            if st.closed {
                return Ok(());
            }
            st.closed = true;
            st.ws
                .close(WebSocketCloseStatusCode::NormalClosure, None, &mut out)
                .map_err(map_ws_err)?
        };
        self.conn.write_bytes(&out[..len]).await?;
        self.conn.flush().await?;
        Ok(())
    }
}

/// Outcome of one synchronous parse attempt.
enum RecvStep {
    /// A complete message is ready.
    Frame(Bytes),
    /// The peer closed the connection.
    Closed,
    /// The buffered bytes do not hold a complete frame; read more.
    NeedMore,
    /// An encoded control frame (Pong or close reply) must be sent.
    Reply {
        len: usize,
        close: bool,
        buf: [u8; WS_CTRL],
    },
}

/// Receiving half of a [`WebSocketTransport`] or [`WebSocketServerTransport`].
pub struct WsRecv<W> {
    state: Arc<RefCell<WasiWsState<W>>>,
    conn: Arc<Connection>,
}

/// Receiving half of a client-role [`WebSocketTransport`].
pub type WebSocketReceiver = WsRecv<WebSocketClient<WsRng>>;
/// Receiving half of a server-role [`WebSocketServerTransport`].
pub type WebSocketServerReceiver = WsRecv<WebSocketServer>;

impl<R, T> WsRecv<WebSocket<R, T>>
where
    R: RngCore,
    T: WebSocketType,
{
    /// Parse buffered bytes. 
    fn parse(&self) -> Result<RecvStep> {
        let mut st = self.state.borrow_mut();
        if st.closed {
            return Ok(RecvStep::Closed);
        }
        let WasiWsState { ws, rx, frame, .. } = &mut *st;
        let mut payload = [0u8; WS_BUF];
        match ws.read(rx.as_slice(), &mut payload) {
            Ok(res) => {
                rx.drain(..res.len_from);
                match res.message_type {
                    WebSocketReceiveMessageType::Text | WebSocketReceiveMessageType::Binary => {
                        frame.extend_from_slice(&payload[..res.len_to]);
                        if res.end_of_message {
                            let message = Bytes::copy_from_slice(frame);
                            frame.clear();
                            Ok(RecvStep::Frame(message))
                        } else {
                            Ok(RecvStep::NeedMore)
                        }
                    }
                    WebSocketReceiveMessageType::Ping => {
                        let mut buf = [0u8; WS_CTRL];
                        let len = ws
                            .write(
                                WebSocketSendMessageType::Pong,
                                true,
                                &payload[..res.len_to],
                                &mut buf,
                            )
                            .map_err(map_ws_err)?;
                        Ok(RecvStep::Reply {
                            len,
                            close: false,
                            buf,
                        })
                    }
                    WebSocketReceiveMessageType::Pong => Ok(RecvStep::NeedMore),
                    WebSocketReceiveMessageType::CloseMustReply => {
                        let mut buf = [0u8; WS_CTRL];
                        let len = ws
                            .write(
                                WebSocketSendMessageType::CloseReply,
                                true,
                                &payload[..res.len_to],
                                &mut buf,
                            )
                            .map_err(map_ws_err)?;
                        Ok(RecvStep::Reply {
                            len,
                            close: true,
                            buf,
                        })
                    }
                    WebSocketReceiveMessageType::CloseCompleted => Ok(RecvStep::Closed),
                }
            }
            Err(WsError::ReadFrameIncomplete) => Ok(RecvStep::NeedMore),
            Err(e) => Err(map_ws_err(e)),
        }
    }
}

#[async_trait(?Send)]
impl<R, T> TransportReceiver for WsRecv<WebSocket<R, T>>
where
    R: RngCore + 'static,
    T: WebSocketType + 'static,
{
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        loop {
            match self.parse()? {
                RecvStep::Frame(message) => return Ok(Some(message)),
                RecvStep::Closed => {
                    self.state.borrow_mut().closed = true;
                    return Ok(None);
                }
                RecvStep::NeedMore => {
                    let mut buf = [0u8; WS_BUF];
                    let n = self.conn.read_bytes(&mut buf).await?;
                    let mut st = self.state.borrow_mut();
                    if n == 0 {
                        st.closed = true;
                        return Ok(None);
                    }
                    st.rx.extend_from_slice(&buf[..n]);
                    if st.rx.len() > WS_MAX_BUFFER {
                        return Err(TransportError::MessageTooLarge {
                            size: st.rx.len(),
                            limit: WS_MAX_BUFFER,
                        });
                    }
                }
                RecvStep::Reply { len, close, buf } => {
                    self.conn.write_bytes(&buf[..len]).await?;
                    self.conn.flush().await?;
                    if close {
                        self.state.borrow_mut().closed = true;
                        return Ok(None);
                    }
                }
            }
        }
    }
}

/// Accepts inbound WASI TCP connections and upgrades them to WebSocket.
pub struct WasiWsListener {
    inner: WasiTcpListener,
}

impl WasiWsListener {
    /// Bind on `addr` (host:port); only the port is used.
    pub fn new(addr: impl Into<String>) -> Result<Self> {
        Ok(Self {
            inner: WasiTcpListener::new(addr)?,
        })
    }

    /// Return the local port this listener is bound to.
    pub fn local_port(&self) -> Result<u16> {
        self.inner.local_port()
    }
}

#[cfg(not(feature = "native"))]
#[async_trait(?Send)]
impl LocalTransportListener for WasiWsListener {
    type Output = WebSocketServerTransport;

    async fn accept(&mut self) -> Result<Option<Self::Output>> {
        let conn = match self.inner.accept_conn().await? {
            Some(conn) => conn,
            None => return Ok(None),
        };
        let mut ws = WebSocketServer::new_server();
        let rx = server_handshake(&conn, &mut ws).await?;
        #[allow(clippy::arc_with_non_send_sync)]
        let state = Arc::new(RefCell::new(WasiWsState {
            ws,
            rx,
            frame: Vec::new(),
            closed: false,
        }));
        Ok(Some(WebSocketServerTransport { state, conn }))
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}
