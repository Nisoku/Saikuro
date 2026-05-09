//! WebSocket transport:  works on both native and wasm32.
//!
//! WebSocket is the only transport that functions in a browser sandbox (apart from in-memory),
//! On native targets it is also useful
//! for remote cross-machine communication through firewalls and proxies that
//! block raw TCP.
use std::net::SocketAddr;

use async_trait::async_trait;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use tracing::{debug, trace, warn};

use crate::{
    error::{Result, TransportError},
    traits::{Transport, TransportListener, TransportReceiver, TransportSender},
};

//  Native implementation

#[cfg(not(target_arch = "wasm32"))]
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

#[cfg(not(target_arch = "wasm32"))]
use saikuro_exec::net::{TcpListener, TcpStream};

/// A WebSocket transport connection.
///
/// On native this wraps `tokio-tungstenite`; the same public API is used on
/// wasm32 but backed by the browser WebSocket.
pub struct WebSocketTransport {
    #[cfg(not(target_arch = "wasm32"))]
    inner: WebSocketStream<MaybeTlsStream<TcpStream>>,
    url: String,
}

impl WebSocketTransport {
    /// Connect to a WebSocket server at `url` (e.g. `"ws://127.0.0.1:9000"`).
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn connect(url: impl Into<String>) -> Result<Self> {
        let url = url.into();
        debug!(%url, "websocket connecting");
        let (ws, _response) = connect_async(&url).await.map_err(|e| {
            TransportError::ConnectionRefused(format!("ws connect to {url} failed: {e}"))
        })?;
        Ok(Self { inner: ws, url })
    }

    /// Construct from an already-upgraded WebSocket stream (server-side accept path).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_stream(ws: WebSocketStream<MaybeTlsStream<TcpStream>>, url: String) -> Self {
        Self { inner: ws, url }
    }
}

impl Transport for WebSocketTransport {
    type Sender = WebSocketSender;
    type Receiver = WebSocketReceiver;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let url = self.url.clone();
            let (sink, stream) = self.inner.split();
            (
                WebSocketSender {
                    inner: sink,
                    url: url.clone(),
                },
                WebSocketReceiver { inner: stream, url },
            )
        }

        #[cfg(target_arch = "wasm32")]
        compile_error!(
            "wasm32 WebSocket split requires wasm-bindgen integration; \
             use the saikuro-wasm crate adapter instead."
        )
    }

    fn description(&self) -> &str {
        "websocket"
    }
}

//  WebSocket transport listener (server-side accept)

/// Listens for inbound TCP connections and upgrades them to WebSocket.
///
/// Implements [`TransportListener`] so it can be used with the same generic
/// accept-loop as TCP and Unix listeners.
#[cfg(not(target_arch = "wasm32"))]
pub struct WsTransportListener {
    inner: TcpListener,
    local_addr: SocketAddr,
}

#[cfg(not(target_arch = "wasm32"))]
impl WsTransportListener {
    /// Bind a TCP listener on the given address for WebSocket upgrades.
    pub async fn bind(addr: SocketAddr) -> Result<Self> {
        let inner = TcpListener::bind(addr).await?;
        let local_addr = inner.local_addr()?;
        debug!(%local_addr, "ws listener bound");
        Ok(Self { inner, local_addr })
    }

    /// Return the address this listener is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl TransportListener for WsTransportListener {
    type Output = WebSocketTransport;

    async fn accept(&mut self) -> Result<Option<Self::Output>> {
        let (stream, peer_addr) = self.inner.accept().await?;
        let url = format!("ws://{peer_addr}");
        let maybe_tls = MaybeTlsStream::Plain(stream);
        match tokio_tungstenite::accept_async(maybe_tls).await {
            Ok(ws_stream) => {
                debug!(peer = %peer_addr, "ws upgrade successful");
                Ok(Some(WebSocketTransport::from_stream(ws_stream, url)))
            }
            Err(e) => {
                warn!(peer = %peer_addr, error = %e, "ws upgrade failed");
                Err(TransportError::ConnectionRefused(format!(
                    "WebSocket upgrade from {peer_addr} failed: {e}"
                )))
            }
        }
    }

    async fn close(&mut self) -> Result<()> {
        debug!(local = %self.local_addr, "ws listener closing");
        Ok(())
    }
}

//  Native Sender / Receiver

#[cfg(not(target_arch = "wasm32"))]
pub struct WebSocketSender {
    inner: futures::stream::SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    url: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl TransportSender for WebSocketSender {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        trace!(url = %self.url, bytes = frame.len(), "ws send");
        self.inner
            .send(Message::Binary(frame.to_vec()))
            .await
            .map_err(|e| TransportError::SendFailed(e.to_string()))
    }

    async fn close(&mut self) -> Result<()> {
        debug!(url = %self.url, "ws sender closing");
        self.inner
            .send(Message::Close(None))
            .await
            .map_err(|e| TransportError::SendFailed(e.to_string()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct WebSocketReceiver {
    inner: futures::stream::SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    url: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl TransportReceiver for WebSocketReceiver {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        loop {
            match self.inner.next().await {
                Some(Ok(Message::Binary(data))) => {
                    trace!(url = %self.url, bytes = data.len(), "ws recv binary");
                    return Ok(Some(Bytes::from(data)));
                }
                Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {
                    // Control frames:  skip transparently.
                    continue;
                }
                Some(Ok(Message::Close(_))) => {
                    debug!(url = %self.url, "ws closed by peer");
                    return Ok(None);
                }
                Some(Ok(other)) => {
                    trace!(url = %self.url, "ws ignoring non-binary frame: {:?}", other);
                    continue;
                }
                Some(Err(e)) => {
                    return Err(TransportError::ReceiveFailed(e.to_string()));
                }
                None => return Ok(None),
            }
        }
    }
}

//  Stub types for wasm32
// These allow the crate to compile on wasm32 even though the full WS
// implementation lives in the saikuro-wasm adapter.  They are never
// instantiated in normal code paths.

#[cfg(target_arch = "wasm32")]
pub struct WebSocketSender;

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl TransportSender for WebSocketSender {
    async fn send(&mut self, _frame: Bytes) -> Result<()> {
        Err(TransportError::NotSupported)
    }
    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
pub struct WebSocketReceiver;

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl TransportReceiver for WebSocketReceiver {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        Err(TransportError::NotSupported)
    }
}
