//! WebAssembly host transport via BroadcastChannel (wasm32 only).
//!
//! Uses uniquely-named `BroadcastChannel`s (negotiated sub-channels) to
//! provide point-to-point transport between WASM contexts in the same
//! origin
//!
//! ## Connection flow
//!
//! 1. **Connector** generates a random connection ID, opens a private
//!    `BroadcastChannel("{base}:{conn_id}")`, and sends a `{ type: "connect",
//!    id: "{conn_id}" }` message on the well-known base channel.
//!
//! 2. **Listener** receives the connect message, opens the same private
//!    channel, and sends a `{ type: "accept", id: "{conn_id}" }` reply.
//!
//! 3. Both sides wrap the private channel in a [`WasmHostTransport`] for
//!    binary frame exchange.
//!
//! Because only the two peers know the private channel name, it behaves
//! like a point-to-point connection even though the underlying primitive
//! is a broadcast bus.
//!
//! ## Backpressure
//!
//! JS `onmessage` callbacks cannot suspend.  If the consumer is slower than
//! the producer, frames are silently dropped at the channel boundary.  The
//! protocol layer above is expected to handle retries (or senders should
//! implement their own flow control).

use async_trait::async_trait;
use bytes::Bytes;
use js_sys::{ArrayBuffer, Reflect, Uint8Array};
use send_wrapper::SendWrapper;
use tracing::trace;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use web_sys::{BroadcastChannel, Crypto, MessageEvent};

use saikuro_exec::mpsc;

use crate::{
    error::{Result, TransportError},
    traits::{
        Transport, TransportConnector, TransportListener, TransportReceiver, TransportSender,
    },
};

use crate::DEFAULT_CHANNEL_CAPACITY;
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

// helpers

/// Generate a 128-bit random hex connection identifier.
fn short_id() -> String {
    let crypto: Crypto = Reflect::get(&js_sys::global(), &"crypto".into())
        .expect("global crypto API available in browser/worker context")
        .unchecked_into();
    let mut buf = [0u8; 16];
    let _ = crypto.get_random_values_with_u8_array(&mut buf);
    buf.iter().fold(String::with_capacity(32), |mut s, b| {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
        s
    })
}

/// Create a JS object literal from key-value pairs.
fn make_obj(pairs: &[(&str, &str)]) -> JsValue {
    let obj = js_sys::Object::new();
    for (k, v) in pairs {
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str(k), &JsValue::from_str(v));
    }
    JsValue::from(obj)
}

/// Try to extract a string field from a JS object-typed JsValue.
fn get_field(val: &JsValue, key: &str) -> Option<String> {
    js_sys::Reflect::get(val, &JsValue::from_str(key))
        .ok()
        .and_then(|v| v.as_string())
}

/// Send binary data as a freshly-allocated `ArrayBuffer` on a channel.
fn send_buffer(channel: &BroadcastChannel, frame: &Bytes) -> Result<()> {
    let len = frame.len() as u32;
    let buffer = ArrayBuffer::new(len);
    let dst = Uint8Array::new(&buffer);
    // Efficient copy: create a view of our WASM-memory data, then use
    // JS TypedArray.set(): one native call, no byte-by-byte overhead.
    // SAFETY: Uint8Array::view creates a zero-copy view into WASM linear
    // memory.  It is only safe when the backing memory is not resized or
    // freed while the view exists.  We consume the view immediately in the
    // `set` call below and never use it again.
    let src = unsafe { Uint8Array::view(frame.as_ref()) };
    dst.set(&JsValue::from(src), 0);
    channel
        .post_message(&JsValue::from(buffer))
        .map_err(|e| TransportError::SendFailed(format!("{e:?}")))
}

// WasmHostTransport

/// A transport backed by a uniquely-named `BroadcastChannel`.
///
/// Constructed internally by [`WasmHostConnector::connect`] and
/// [`WasmHostListener::accept`].  After construction, call
/// [`Transport::split`] to obtain the sender/receiver halves.
pub struct WasmHostTransport {
    sender: WasmHostSender,
    receiver: WasmHostReceiver,
}

impl WasmHostTransport {
    /// Wrap a `BroadcastChannel` as a transport.
    ///
    /// Installs an `onmessage` handler that pushes incoming binary frames
    /// into an MPSC channel for async consumption.
    pub fn new(channel: BroadcastChannel, label: impl Into<String>) -> Self {
        let label = label.into();
        let (tx, rx) = mpsc::channel::<Bytes>(DEFAULT_CHANNEL_CAPACITY);

        let bridge_tx = tx;
        let handler: Closure<dyn FnMut(MessageEvent)> = Closure::new(move |event: MessageEvent| {
            let data = event.data();
            let bytes = if let Some(buf) = data.dyn_ref::<ArrayBuffer>() {
                Uint8Array::new(buf).to_vec()
            } else if let Some(arr) = data.dyn_ref::<Uint8Array>() {
                arr.to_vec()
            } else {
                return;
            };
            let _ = bridge_tx.try_send(Bytes::from(bytes));
        });
        channel.set_onmessage(Some(handler.as_ref().unchecked_ref()));

        WasmHostTransport {
            sender: WasmHostSender {
                channel: SendWrapper::new(channel.clone()),
                label: label.clone(),
            },
            receiver: WasmHostReceiver {
                channel: SendWrapper::new(channel),
                rx,
                _handler: SendWrapper::new(handler),
                label,
            },
        }
    }
}

impl Transport for WasmHostTransport {
    type Sender = WasmHostSender;
    type Receiver = WasmHostReceiver;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        (self.sender, self.receiver)
    }

    fn description(&self) -> &str {
        "wasm-host"
    }
}

// WasmHostSender

/// Sending half of a [`WasmHostTransport`].
pub struct WasmHostSender {
    channel: SendWrapper<BroadcastChannel>,
    label: String,
}

#[async_trait]
impl TransportSender for WasmHostSender {
    async fn send(&mut self, frame: Bytes) -> Result<()> {
        trace!(label = %self.label, bytes = frame.len(), "wasm-host send");
        send_buffer(&self.channel, &frame)
    }

    async fn close(&mut self) -> Result<()> {
        trace!(label = %self.label, "wasm-host sender closing");
        Ok(())
    }
}

// WasmHostReceiver

/// Receiving half of a [`WasmHostTransport`].
///
/// Owns the channel lifecycle: on drop the `onmessage` handler is removed
/// and the channel is closed.
pub struct WasmHostReceiver {
    channel: SendWrapper<BroadcastChannel>,
    rx: mpsc::Receiver<Bytes>,
    _handler: SendWrapper<Closure<dyn FnMut(MessageEvent)>>,
    label: String,
}

impl Drop for WasmHostReceiver {
    fn drop(&mut self) {
        self.channel.set_onmessage(None);
        self.channel.close();
    }
}

#[async_trait]
impl TransportReceiver for WasmHostReceiver {
    async fn recv(&mut self) -> Result<Option<Bytes>> {
        let result = self.rx.recv().await;
        match &result {
            Some(bytes) => trace!(label = %self.label, bytes = bytes.len(), "wasm-host recv"),
            None => trace!(label = %self.label, "wasm-host channel closed"),
        }
        Ok(result)
    }
}

// WasmHostConnector

/// Initiates an outgoing WASM host connection.
///
/// The connector opens a private `BroadcastChannel` and sends a connect
/// request on the well-known rendezvous channel.  Once the listener replies
/// on the private channel the connection is established.
pub struct WasmHostConnector {
    channel_name: String,
}

impl WasmHostConnector {
    /// Create a connector that will rendezvous on `channel_name`.
    pub fn new(channel_name: impl Into<String>) -> Self {
        Self {
            channel_name: channel_name.into(),
        }
    }
}

#[async_trait]
impl TransportConnector for WasmHostConnector {
    type Output = WasmHostTransport;

    async fn connect(&self) -> Result<Self::Output> {
        let conn_id = short_id();
        let private_name = format!("{}:{}", self.channel_name, conn_id);

        // Open private channel FIRST so we're listening before the
        // connect request reaches the listener.
        let private = BroadcastChannel::new(&private_name)
            .map_err(|e| TransportError::ConnectionLost(format!("{e:?}")))?;

        // Signal channel: listener's accept reply will unblock us.
        let (signal_tx, mut signal_rx) = mpsc::channel::<()>(1);

        // Temporary accept handler on the private channel.
        // Scoped in a block so the raw `Closure` is consumed into the
        // `SendWrapper` BEFORE any await point.
        let _handler_guard: SendWrapper<Closure<dyn FnMut(MessageEvent)>> = {
            let h: Closure<dyn FnMut(MessageEvent)> = Closure::new({
                let signal = signal_tx;
                move |_event: MessageEvent| {
                    let _ = signal.try_send(());
                }
            });
            private.set_onmessage(Some(h.as_ref().unchecked_ref()));
            SendWrapper::new(h)
        };

        // Send connect request on the base channel.
        let base = BroadcastChannel::new(&self.channel_name)
            .map_err(|e| TransportError::ConnectionLost(format!("{e:?}")))?;
        let msg = make_obj(&[("type", "connect"), ("id", &conn_id)]);
        base.post_message(&msg)
            .map_err(|e| TransportError::ConnectionLost(format!("{e:?}")))?;

        // Wait for accept with timeout.
        let result = saikuro_exec::timeout(CONNECT_TIMEOUT, signal_rx.recv()).await;

        match result {
            Ok(Some(())) => {
                // Accept received.  WasmHostTransport::new replaces the
                // accept handler with the real data handler.
                Ok(WasmHostTransport::new(private, conn_id))
            }
            Ok(None) => Err(TransportError::ConnectionLost(
                "accept channel closed".into(),
            )),
            Err(_) => Err(TransportError::ConnectionLost("connect timeout".into())),
        }
    }
}

// WasmHostListener

/// Accepts incoming WASM host connections on a well-known channel name.
///
/// Opens a `BroadcastChannel` on the rendezvous name and installs an
/// `onmessage` handler that queues incoming connect requests.  Each call
/// to [`accept`](TransportListener::accept) pops the next request,
/// opens the corresponding private channel, sends an accept reply, and
/// returns the transport.
pub struct WasmHostListener {
    base_name: String,
    connect_rx: mpsc::Receiver<String>,
    _base_channel: SendWrapper<BroadcastChannel>,
    _handler: SendWrapper<Closure<dyn FnMut(MessageEvent)>>,
    closed: bool,
}

impl WasmHostListener {
    /// Start listening for connections on `channel_name`.
    pub fn new(channel_name: impl Into<String>) -> Result<Self> {
        let base_name: String = channel_name.into();
        let base = BroadcastChannel::new(&base_name)
            .map_err(|e| TransportError::ConnectionLost(format!("{e:?}")))?;

        let (tx, rx) = mpsc::channel::<String>(32);

        let handler_tx = tx;
        let handler: Closure<dyn FnMut(MessageEvent)> = Closure::new(move |event: MessageEvent| {
            let data = event.data();
            let msg_type = get_field(&data, "type");
            if msg_type.as_deref() != Some("connect") {
                return;
            }
            if let Some(conn_id) = get_field(&data, "id") {
                let _ = handler_tx.try_send(conn_id);
            }
        });
        base.set_onmessage(Some(handler.as_ref().unchecked_ref()));

        Ok(Self {
            base_name,
            connect_rx: rx,
            _base_channel: SendWrapper::new(base),
            _handler: SendWrapper::new(handler),
            closed: false,
        })
    }
}

#[async_trait]
impl TransportListener for WasmHostListener {
    type Output = WasmHostTransport;

    async fn accept(&mut self) -> Result<Option<Self::Output>> {
        if self.closed {
            return Ok(None);
        }
        let conn_id = match self.connect_rx.recv().await {
            Some(id) => id,
            None => return Ok(None),
        };

        let private_name = format!("{}:{}", self.base_name, conn_id);
        let private = BroadcastChannel::new(&private_name)
            .map_err(|e| TransportError::ConnectionLost(format!("{e:?}")))?;

        // Send accept reply on the private channel.
        let msg = make_obj(&[("type", "accept"), ("id", &conn_id)]);
        private
            .post_message(&msg)
            .map_err(|e| TransportError::ConnectionLost(format!("{e:?}")))?;

        Ok(Some(WasmHostTransport::new(private, conn_id)))
    }

    async fn close(&mut self) -> Result<()> {
        self._base_channel.set_onmessage(None);
        self._base_channel.close();
        self.closed = true;
        Ok(())
    }
}
