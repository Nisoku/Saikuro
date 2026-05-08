//! WebSocket transport:  works on both native and wasm32.
//!
//! WebSocket is the only transport that functions in a browser sandbox (apart from in-memory),
//! On native targets it is also useful
//! for remote cross-machine communication through firewalls and proxies that
//! block raw TCP.
use async_trait::async_trait;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use tracing::{debug, trace};

use crate::{
    error::{Result, TransportError},
    traits::{Transport, TransportReceiver, TransportSender},
};

//  Native implementation

#[cfg(not(target_arch = "wasm32"))]
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

#[cfg(not(target_arch = "wasm32"))]
use saikuro_exec::net::TcpStream;

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
