//! WebSocket transport:  works on both native and wasm32.
//!
//! On native targets the implementation wraps `tokio-tungstenite` for a
//! full-duplex, TLS-capable WebSocket over TCP.
//!
//! On wasm32 targets the implementation wraps `web-sys::WebSocket` (the
//! browser's native WebSocket API) and bridges its event-driven callbacks
//! into async channels, giving the same [`Transport`]-trait interface.

use async_trait::async_trait;
use bytes::Bytes;
use tracing::{debug, trace};

use crate::{
    error::{Result, TransportError},
    traits::{Transport, TransportReceiver, TransportSender},
};

#[cfg(not(target_arch = "wasm32"))]
use crate::traits::TransportListener;

#[cfg(target_arch = "wasm32")]
use crate::DEFAULT_CHANNEL_CAPACITY;

// Native (tokio-tungstenite) implementation
#[cfg(not(target_arch = "wasm32"))]
use std::net::SocketAddr;

#[cfg(not(target_arch = "wasm32"))]
use futures::{SinkExt, StreamExt};

#[cfg(not(target_arch = "wasm32"))]
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

#[cfg(not(target_arch = "wasm32"))]
use saikuro_exec::net::{TcpListener, TcpStream};

/// A WebSocket transport connection.
///
/// On native this wraps `tokio-tungstenite`; on wasm32 it wraps the browser's
/// `WebSocket` API.  Same public API on both platforms.
pub struct WebSocketTransport {
    #[cfg(not(target_arch = "wasm32"))]
    inner: WebSocketStream<MaybeTlsStream<TcpStream>>,
    #[cfg(target_arch = "wasm32")]
    ws: send_wrapper::SendWrapper<web_sys::WebSocket>,
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

    /// Connect to a WebSocket server using the browser WebSocket API.
    #[cfg(target_arch = "wasm32")]
    pub async fn connect(url: impl Into<String>) -> Result<Self> {
        use send_wrapper::SendWrapper;
        use std::sync::{Arc, Mutex};
        use wasm_bindgen::{closure::Closure, JsCast};
        use web_sys::{BinaryType, ErrorEvent, Event};

        let url = url.into();
        debug!(%url, "wasm websocket connecting");

        let ws = web_sys::WebSocket::new(&url)
            .map_err(|e| TransportError::ConnectionRefused(format!("{e:?}")))?;
        ws.set_binary_type(BinaryType::Arraybuffer);

        let (tx, rx) = saikuro_exec::oneshot::channel::<Result<()>>();
        let shared: Arc<Mutex<Option<saikuro_exec::oneshot::Sender<Result<()>>>>> =
            Arc::new(Mutex::new(Some(tx)));

        let open_shared = shared.clone();
        let onopen = Closure::<dyn FnMut(Event)>::new(move |_: Event| {
            if let Some(s) = open_shared.lock().unwrap_or_else(|e| e.into_inner()).take() {
                let _ = s.send(Ok(()));
            }
        });
        ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));

        let error_shared = shared;
        let onerror = Closure::<dyn FnMut(ErrorEvent)>::new(move |e: ErrorEvent| {
            if let Some(s) = error_shared
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
            {
                let _ = s.send(Err(TransportError::ConnectionRefused(e.message())));
            }
        });
        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        let result = saikuro_exec::timeout(std::time::Duration::from_secs(30), async {
            rx.await.unwrap_or(Err(TransportError::ConnectionRefused(
                "connection cancelled".into(),
            )))
        })
        .await;

        ws.set_onopen(None);
        ws.set_onerror(None);

        match result {
            Ok(Ok(())) => {
                debug!(%url, "wasm websocket connected");
                Ok(Self {
                    ws: SendWrapper::new(ws),
                    url,
                })
            }
            Ok(Err(e)) => {
                ws.close().ok();
                Err(e)
            }
            Err(_) => {
                ws.close().ok();
                Err(TransportError::ConnectionRefused("connect timeout".into()))
            }
        }
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
        {
            use js_sys::{ArrayBuffer, Uint8Array};
            use send_wrapper::SendWrapper;
            use wasm_bindgen::{closure::Closure, JsCast};
            use web_sys::{CloseEvent, ErrorEvent, MessageEvent};

            type WsEvent = std::result::Result<Option<Bytes>, TransportError>;

            let (tx, rx) = saikuro_exec::mpsc::channel::<WsEvent>(DEFAULT_CHANNEL_CAPACITY);

            let ws = self.ws.take();
            let ws_for_receiver = ws.clone();

            let msg_tx = tx.clone();
            let onmsg = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
                let data = event.data();
                let bytes = if let Some(buf) = data.dyn_ref::<ArrayBuffer>() {
                    Uint8Array::new(buf).to_vec()
                } else if let Some(arr) = data.dyn_ref::<Uint8Array>() {
                    arr.to_vec()
                } else {
                    return;
                };
                let _ = msg_tx.try_send(Ok(Some(Bytes::from(bytes))));
            });
            let _ = ws_for_receiver.set_onmessage(Some(onmsg.as_ref().unchecked_ref()));

            let close_tx = tx.clone();
            let onclose = Closure::<dyn FnMut(CloseEvent)>::new(move |_: CloseEvent| {
                let _ = close_tx.try_send(Ok(None));
            });
            let _ = ws_for_receiver.set_onclose(Some(onclose.as_ref().unchecked_ref()));

            let error_tx = tx;
            let onerror = Closure::<dyn FnMut(ErrorEvent)>::new(move |e: ErrorEvent| {
                let _ = error_tx.try_send(Err(TransportError::ReceiveFailed(e.message())));
            });
            let _ = ws_for_receiver.set_onerror(Some(onerror.as_ref().unchecked_ref()));

            let url = self.url;

            (
                WebSocketSender {
                    ws: SendWrapper::new(ws),
                    url: url.clone(),
                },
                WebSocketReceiver {
                    ws: SendWrapper::new(ws_for_receiver),
                    rx,
                    _onmsg: SendWrapper::new(onmsg),
                    _onclose: SendWrapper::new(onclose),
                    _onerror: SendWrapper::new(onerror),
                    url,
                },
            )
        }
    }

    fn description(&self) -> &str {
        "websocket"
    }
}

// WebSocket transport listener (server-side accept, native only)
/// Listens for inbound TCP connections and upgrades them to WebSocket.
///
/// Implements [`TransportListener`] so it can be used with the same generic
/// accept-loop as TCP and Unix listeners.  Not available on wasm32 (browsers
/// cannot listen for TCP connections).
#[cfg(not(target_arch = "wasm32"))]
pub struct WsTransportListener {
    inner: Option<TcpListener>,
    local_addr: SocketAddr,
}

#[cfg(not(target_arch = "wasm32"))]
impl WsTransportListener {
    /// Bind a TCP listener on the given address for WebSocket upgrades.
    pub async fn bind(addr: SocketAddr) -> Result<Self> {
        let inner = TcpListener::bind(addr).await?;
        let local_addr = inner.local_addr()?;
        debug!(%local_addr, "ws listener bound");
        Ok(Self {
            inner: Some(inner),
            local_addr,
        })
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
        let inner = self
            .inner
            .as_ref()
            .ok_or_else(|| TransportError::ConnectionRefused("listener closed".into()))?;
        let (stream, peer_addr) = inner.accept().await?;
        let url = format!("ws://{peer_addr}");
        let maybe_tls = MaybeTlsStream::Plain(stream);
        match tokio_tungstenite::accept_async(maybe_tls).await {
            Ok(ws_stream) => {
                debug!(peer = %peer_addr, "ws upgrade successful");
                Ok(Some(WebSocketTransport::from_stream(ws_stream, url)))
            }
            Err(e) => {
                tracing::warn!(peer = %peer_addr, error = %e, "ws upgrade failed");
                Err(TransportError::ConnectionRefused(format!(
                    "WebSocket upgrade from {peer_addr} failed: {e}"
                )))
            }
        }
    }

    async fn close(&mut self) -> Result<()> {
        debug!(local = %self.local_addr, "ws listener closing");
        drop(self.inner.take());
        Ok(())
    }
}

// Native Sender / Receiver
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

// WASM Sender / Receiver  (web-sys::WebSocket)
/// Sending half of a WASM WebSocket transport.
///
/// Sends binary frames via [`web_sys::WebSocket::send_with_array_buffer`].
#[cfg(target_arch = "wasm32")]
pub struct WebSocketSender {
    ws: send_wrapper::SendWrapper<web_sys::WebSocket>,
    url: String,
}

#[cfg(target_arch = "wasm32")]
#[async_trait]
impl TransportSender for WebSocketSender {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        use js_sys::{ArrayBuffer, Uint8Array};
        use wasm_bindgen::JsValue;
        trace!(url = %self.url, bytes = frame.len(), "wasm ws send");
        let len = frame.len() as u32;
        let buffer = ArrayBuffer::new(len);
        let dst = Uint8Array::new(&buffer);
        let src = unsafe { Uint8Array::view(frame.as_ref()) };
        dst.set(&JsValue::from(src), 0);
        self.ws
            .send_with_array_buffer(&buffer)
            .map_err(|e| TransportError::SendFailed(format!("{e:?}")))
    }

    async fn close(&mut self) -> Result<()> {
        debug!(url = %self.url, "wasm ws sender closing");
        self.ws
            .close()
            .map_err(|e| TransportError::SendFailed(format!("{e:?}")))
    }
}

/// Receiving half of a WASM WebSocket transport.
///
/// Bridges the browser's event-driven [`web_sys::WebSocket`] (`onmessage`,
/// `onclose`, `onerror`) into an async MPSC channel for the
/// [`TransportReceiver`] trait.
#[cfg(target_arch = "wasm32")]
pub struct WebSocketReceiver {
    ws: send_wrapper::SendWrapper<web_sys::WebSocket>,
    rx: saikuro_exec::mpsc::Receiver<std::result::Result<Option<Bytes>, TransportError>>,
    _onmsg:
        send_wrapper::SendWrapper<wasm_bindgen::closure::Closure<dyn FnMut(web_sys::MessageEvent)>>,
    _onclose:
        send_wrapper::SendWrapper<wasm_bindgen::closure::Closure<dyn FnMut(web_sys::CloseEvent)>>,
    _onerror:
        send_wrapper::SendWrapper<wasm_bindgen::closure::Closure<dyn FnMut(web_sys::ErrorEvent)>>,
    url: String,
}

#[cfg(target_arch = "wasm32")]
impl Drop for WebSocketReceiver {
    fn drop(&mut self) {
        self.ws.set_onmessage(None);
        self.ws.set_onclose(None);
        self.ws.set_onerror(None);
        let _ = self.ws.close();
    }
}

#[cfg(target_arch = "wasm32")]
#[async_trait]
impl TransportReceiver for WebSocketReceiver {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        match self.rx.recv().await {
            Some(Ok(opt)) => {
                if opt.is_some() {
                    trace!(url = %self.url, bytes = opt.as_ref().unwrap().len(), "wasm ws recv");
                } else {
                    debug!(url = %self.url, "wasm ws closed by peer");
                }
                Ok(opt)
            }
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }
}
