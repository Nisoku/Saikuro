#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;
use async_trait::async_trait;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic_util::Arc;
use saikuro_event::{LogLevel, LogRecord};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use saikuro_net::net::{TcpListener, TcpStream};

use std::net::SocketAddr;

use crate::shared::{
    error::{Result, TransportError},
    traits::{Transport, TransportListener, TransportReceiver, TransportSender},
};

/// Wraps `tokio-tungstenite`.
pub struct WebSocketTransport {
    inner: WebSocketStream<MaybeTlsStream<TcpStream>>,
    url: String,
    log: Arc<dyn saikuro_event::LogSink>,
}

impl WebSocketTransport {
    /// Connect to a WebSocket server at `url` (e.g. `"ws://127.0.0.1:9000"`).
    pub async fn connect(
        url: impl Into<String>,
        log: Arc<dyn saikuro_event::LogSink>,
    ) -> Result<Self> {
        let url = url.into();
        let mut record = LogRecord::now(
            LogLevel::Debug,
            "saikuro.transport.websocket",
            "websocket connecting",
        );
        record.set_context("url", url.clone());
        log.emit(&record).await;
        let (ws, _response) = connect_async(&url).await.map_err(|e| {
            TransportError::ConnectionRefused(format!("ws connect to {url} failed: {e}"))
        })?;
        Ok(Self {
            inner: ws,
            url,
            log,
        })
    }

    /// Construct from an already-upgraded WebSocket stream (server-side accept path).
    pub fn from_stream(
        ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
        url: String,
        log: Arc<dyn saikuro_event::LogSink>,
    ) -> Self {
        Self {
            inner: ws,
            url,
            log,
        }
    }
}

impl Transport for WebSocketTransport {
    type Sender = WebSocketSender;
    type Receiver = WebSocketReceiver;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        let url = self.url.clone();
        let log = self.log;
        let (sink, stream) = self.inner.split();
        (
            WebSocketSender {
                inner: sink,
                url: url.clone(),
                log: log.clone(),
            },
            WebSocketReceiver {
                inner: stream,
                url,
                log,
            },
        )
    }

    fn description(&self) -> &str {
        "websocket"
    }
}

// WebSocket transport listener (server-side accept, native only)
/// Listens for inbound TCP connections and upgrades them to WebSocket.
pub struct WsTransportListener {
    inner: Option<TcpListener>,
    local_addr: SocketAddr,
    log: Arc<dyn saikuro_event::LogSink>,
}

impl WsTransportListener {
    /// Bind a TCP listener on the given address for WebSocket upgrades.
    pub async fn bind(addr: SocketAddr, log: Arc<dyn saikuro_event::LogSink>) -> Result<Self> {
        let inner = TcpListener::bind(addr).await?;
        let local_addr = inner.local_addr()?;
        let mut record = LogRecord::now(
            LogLevel::Debug,
            "saikuro.transport.websocket",
            "ws listener bound",
        );
        record.set_context("local_addr", alloc::format!("{}", local_addr));
        log.emit(&record).await;
        Ok(Self {
            inner: Some(inner),
            local_addr,
            log,
        })
    }

    /// Return the address this listener is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

#[async_trait]
impl TransportListener for WsTransportListener {
    type Output = WebSocketTransport;

    async fn accept(&mut self) -> Result<Option<Self::Output>> {
        let inner = self
            .inner
            .as_ref()
            .ok_or_else(|| TransportError::ConnectionRefused("listener closed".into()))?;
        let (stream, peer_addr) = inner.accept().await?;
        let url = format!("ws://{peer_addr}");
        let maybe_tls = MaybeTlsStream::Plain(stream);
        match tokio_tungstenite::accept_async(maybe_tls).await {
            Ok(ws_stream) => {
                let mut record = LogRecord::now(
                    LogLevel::Debug,
                    "saikuro.transport.websocket",
                    "ws upgrade successful",
                );
                record.set_context("peer", alloc::format!("{}", peer_addr));
                self.log.emit(&record).await;
                Ok(Some(WebSocketTransport::from_stream(
                    ws_stream,
                    url,
                    self.log.clone(),
                )))
            }
            Err(e) => {
                let mut record = LogRecord::now(
                    LogLevel::Warn,
                    "saikuro.transport.websocket",
                    "ws upgrade failed",
                );
                record.set_context("peer", alloc::format!("{}", peer_addr));
                record.set_context("error", alloc::format!("{}", e));
                self.log.emit(&record).await;
                Err(TransportError::ConnectionRefused(format!(
                    "WebSocket upgrade from {peer_addr} failed: {e}"
                )))
            }
        }
    }

    async fn close(&mut self) -> Result<()> {
        let mut record = LogRecord::now(
            LogLevel::Debug,
            "saikuro.transport.websocket",
            "ws listener closing",
        );
        record.set_context("local_addr", alloc::format!("{}", self.local_addr));
        self.log.emit(&record).await;
        drop(self.inner.take());
        Ok(())
    }
}

/// Sending half of a native WebSocket transport.
pub struct WebSocketSender {
    inner: futures::stream::SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    url: String,
    log: Arc<dyn saikuro_event::LogSink>,
}

#[async_trait]
impl TransportSender for WebSocketSender {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        let mut record = LogRecord::now(LogLevel::Trace, "saikuro.transport.websocket", "ws send");
        record.set_context("url", self.url.clone());
        record.set_context("bytes", frame.len() as u64);
        self.log.emit(&record).await;
        self.inner
            .send(Message::Binary(frame))
            .await
            .map_err(|e| TransportError::SendFailed(e.to_string()))
    }

    async fn close(&mut self) -> Result<()> {
        let mut record = LogRecord::now(
            LogLevel::Debug,
            "saikuro.transport.websocket",
            "ws sender closing",
        );
        record.set_context("url", self.url.clone());
        self.log.emit(&record).await;
        self.inner
            .send(Message::Close(None))
            .await
            .map_err(|e| TransportError::SendFailed(e.to_string()))
    }
}

/// Receiving half of a native WebSocket transport.
pub struct WebSocketReceiver {
    inner: futures::stream::SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    url: String,
    log: Arc<dyn saikuro_event::LogSink>,
}

#[async_trait]
impl TransportReceiver for WebSocketReceiver {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        loop {
            match self.inner.next().await {
                Some(Ok(Message::Binary(data))) => {
                    let mut record = LogRecord::now(
                        LogLevel::Trace,
                        "saikuro.transport.websocket",
                        "ws recv binary",
                    );
                    record.set_context("url", self.url.clone());
                    record.set_context("bytes", data.len() as u64);
                    self.log.emit(&record).await;
                    return Ok(Some(data));
                }
                Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {
                    continue;
                }
                Some(Ok(Message::Close(_))) => {
                    let mut record = LogRecord::now(
                        LogLevel::Debug,
                        "saikuro.transport.websocket",
                        "ws closed by peer",
                    );
                    record.set_context("url", self.url.clone());
                    self.log.emit(&record).await;
                    return Ok(None);
                }
                Some(Ok(other)) => {
                    let mut record = LogRecord::now(
                        LogLevel::Trace,
                        "saikuro.transport.websocket",
                        "ws ignoring non-binary frame",
                    );
                    record.set_context("url", self.url.clone());
                    record.set_context("frame_type", alloc::format!("{:?}", other));
                    self.log.emit(&record).await;
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
