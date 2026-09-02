use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic_util::Arc;
#[cfg(target_has_atomic = "ptr")]
use saikuro_core::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use core::cell::RefCell;

use crate::shared::error::{Result, TransportError};
use crate::shared::traits::{Transport, TransportReceiver, TransportSender};
use crate::wasi::tcp::backend::Connection;
use crate::wasi::tcp::WasiConn;

use embedded_websocket::framer::{Framer, ReadResult, Stream as WsStream};
use embedded_websocket::{
    WebSocketClient, WebSocketCloseStatusCode, WebSocketOptions, WebSocketSendMessageType,
};

/// Internal framing buffer size.
const WS_BUF: usize = 4096;

struct WsRng;

impl rand_core_06::RngCore for WsRng {
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

/// Adapts a connected WASI socket to the byte-stream interface the
/// `embedded-websocket` sync framer requires.
struct WasiWsConn {
    conn: Arc<Connection>,
}

impl WsStream<TransportError> for WasiWsConn {
    fn read(&mut self, buf: &mut [u8]) -> core::result::Result<usize, TransportError> {
        self.conn.read_bytes(buf)
    }

    fn write_all(&mut self, buf: &[u8]) -> core::result::Result<(), TransportError> {
        self.conn.write_bytes(buf)
    }
}

/// Shared per-connection state.  The frame and parse buffers are owned here so
/// sender and receiver can share one socket through a single `RefCell`.
struct WasiWsState {
    ws: WebSocketClient<WsRng>,
    conn: WasiWsConn,
    read_buf: [u8; WS_BUF],
    write_buf: [u8; WS_BUF],
    read_cursor: usize,
}

fn ws_err(e: embedded_websocket::framer::FramerError<TransportError>) -> TransportError {
    use embedded_websocket::framer::FramerError;
    match e {
        FramerError::Io(e) => TransportError::ConnectionLost(format!("ws io: {e:?}")),
        FramerError::WebSocket(ws) => TransportError::ReceiveFailed(format!("ws: {ws:?}")),
        FramerError::HttpHeader(h) => {
            TransportError::ConnectionRefused(format!("ws handshake http: {h:?}"))
        }
        FramerError::FrameTooLarge(n) => TransportError::MessageTooLarge {
            size: n,
            limit: WS_BUF,
        },
        FramerError::Utf8(u) => TransportError::ReceiveFailed(format!("ws utf8: {u}")),
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

/// A `no_std` WebSocket client transport backed by a WASI socket.
pub struct WebSocketTransport {
    state: Arc<RefCell<WasiWsState>>,
}

impl WebSocketTransport {
    /// Open a WebSocket connection to `url` (`ws://host[:port][/path]`).
    pub async fn connect(url: impl Into<String>) -> Result<Self> {
        let url = url.into();
        let (host, port, path) = parse_ws_url(&url)?;
        let conn = crate::wasi::tcp::backend::connect(&format!("{host}:{port}"))?;
        let mut state = WasiWsState {
            ws: WebSocketClient::new_client(WsRng),
            conn: WasiWsConn { conn },
            read_buf: [0u8; WS_BUF],
            write_buf: [0u8; WS_BUF],
            read_cursor: 0,
        };
        let options = WebSocketOptions {
            path: &path,
            host: &host,
            origin: &host,
            sub_protocols: None,
            additional_headers: None,
        };
        {
            let mut framer = Framer::new(
                &mut state.read_buf,
                &mut state.read_cursor,
                &mut state.write_buf,
                &mut state.ws,
            );
            framer.connect(&mut state.conn, &options).map_err(ws_err)?;
        }
        Ok(Self {
            state: Arc::new(RefCell::new(state)),
        })
    }
}

impl Transport for WebSocketTransport {
    type Sender = WebSocketSender;
    type Receiver = WebSocketReceiver;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        let s = self.state.clone();
        (
            WebSocketSender { state: s.clone() },
            WebSocketReceiver { state: s },
        )
    }

    fn description(&self) -> &str {
        "websocket"
    }
}

/// Sending half of a [`WebSocketTransport`].
pub struct WebSocketSender {
    state: Arc<RefCell<WasiWsState>>,
}

#[async_trait(?Send)]
impl TransportSender for WebSocketSender {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        let mut st = self.state.borrow_mut();
        let st: &mut WasiWsState = &mut *st;
        let mut framer = Framer::new(
            &mut st.read_buf,
            &mut st.read_cursor,
            &mut st.write_buf,
            &mut st.ws,
        );
        framer
            .write(&mut st.conn, WebSocketSendMessageType::Binary, true, &frame)
            .map_err(ws_err)?;
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        let mut st = self.state.borrow_mut();
        let st: &mut WasiWsState = &mut *st;
        let mut framer = Framer::new(
            &mut st.read_buf,
            &mut st.read_cursor,
            &mut st.write_buf,
            &mut st.ws,
        );
        framer
            .close(&mut st.conn, WebSocketCloseStatusCode::NormalClosure, None)
            .map_err(ws_err)?;
        Ok(())
    }
}

/// Receiving half of a [`WebSocketTransport`].
pub struct WebSocketReceiver {
    state: Arc<RefCell<WasiWsState>>,
}

#[async_trait(?Send)]
impl TransportReceiver for WebSocketReceiver {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        let mut st = self.state.borrow_mut();
        let st: &mut WasiWsState = &mut *st;
        let mut frame_buf = [0u8; WS_BUF];
        loop {
            let mut framer = Framer::new(
                &mut st.read_buf,
                &mut st.read_cursor,
                &mut st.write_buf,
                &mut st.ws,
            );
            match framer.read(&mut st.conn, &mut frame_buf).map_err(ws_err)? {
                ReadResult::Binary(b) => return Ok(Some(Bytes::copy_from_slice(b))),
                ReadResult::Text(t) => return Ok(Some(Bytes::copy_from_slice(t.as_bytes()))),
                ReadResult::Pong(_) => continue,
                ReadResult::Closed => return Ok(None),
            }
        }
    }
}
