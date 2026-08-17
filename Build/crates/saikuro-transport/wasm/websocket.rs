use alloc::rc::Rc;
use alloc::string::String;
use core::cell::RefCell;

use async_trait::async_trait;
use bytes::Bytes;
use send_wrapper::SendWrapper;
use tracing::{debug, trace};
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{BinaryType, CloseEvent, ErrorEvent, Event, MessageEvent};

use saikuro_exec::mpsc;
use saikuro_exec::oneshot;
use saikuro_exec::timeout;

use crate::shared::error::{Result, TransportError};
use crate::shared::traits::{Transport, TransportReceiver, TransportSender};
use crate::DEFAULT_CHANNEL_CAPACITY;

/// A WebSocket transport connection (browser).
pub struct WebSocketTransport {
    ws: SendWrapper<web_sys::WebSocket>,
    url: String,
}

impl WebSocketTransport {
    /// Connect to a WebSocket server using the browser WebSocket API.
    pub async fn connect(url: impl Into<String>) -> Result<Self> {
        let url = url.into();
        debug!(%url, "wasm websocket connecting");

        let ws = web_sys::WebSocket::new(&url)
            .map_err(|e| TransportError::ConnectionRefused(format!("{e:?}")))?;
        ws.set_binary_type(BinaryType::Arraybuffer);

        let (tx, rx) = oneshot::channel::<Result<()>>();
        let shared: Rc<RefCell<Option<oneshot::Sender<Result<()>>>>> =
            Rc::new(RefCell::new(Some(tx)));

        let open_shared = Rc::clone(&shared);
        let onopen = Closure::<dyn FnMut(Event)>::new(move |_: Event| {
            if let Some(s) = open_shared.borrow_mut().take() {
                let _ = s.send(Ok(()));
            }
        });
        ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));

        let error_shared = shared;
        let onerror = Closure::<dyn FnMut(ErrorEvent)>::new(move |e: ErrorEvent| {
            if let Some(s) = error_shared.borrow_mut().take() {
                let _ = s.send(Err(TransportError::ConnectionRefused(e.message())));
            }
        });
        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        let result = timeout(core::time::Duration::from_secs(30), async {
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
}

impl Transport for WebSocketTransport {
    type Sender = WebSocketSender;
    type Receiver = WebSocketReceiver;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        use js_sys::{ArrayBuffer, Uint8Array};

        type WsEvent = core::result::Result<Option<Bytes>, TransportError>;

        let (tx, rx) = mpsc::channel::<WsEvent>(DEFAULT_CHANNEL_CAPACITY);

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

    fn description(&self) -> &str {
        "websocket"
    }
}

/// Sending half of a WASM WebSocket transport.
///
/// Sends binary frames via [`web_sys::WebSocket::send_with_array_buffer`].
pub struct WebSocketSender {
    ws: SendWrapper<web_sys::WebSocket>,
    url: String,
}

#[async_trait(?Send)]
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
pub struct WebSocketReceiver {
    ws: SendWrapper<web_sys::WebSocket>,
    rx: mpsc::Receiver<core::result::Result<Option<Bytes>, TransportError>>,
    _onmsg: SendWrapper<Closure<dyn FnMut(MessageEvent)>>,
    _onclose: SendWrapper<Closure<dyn FnMut(CloseEvent)>>,
    _onerror: SendWrapper<Closure<dyn FnMut(ErrorEvent)>>,
    url: String,
}

impl Drop for WebSocketReceiver {
    fn drop(&mut self) {
        self.ws.set_onmessage(None);
        self.ws.set_onclose(None);
        self.ws.set_onerror(None);
        let _ = self.ws.close();
    }
}

#[async_trait(?Send)]
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
