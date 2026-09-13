use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use async_trait::async_trait;
use bytes::Bytes;
use core::cell::{Cell, RefCell};
use core::fmt::Write;
use core::time::Duration;
use js_sys::{ArrayBuffer, Reflect, Uint8Array};
use send_wrapper::SendWrapper;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use web_sys::{BroadcastChannel, Crypto, MessageEvent};

use saikuro_core::Arc;
use saikuro_exec::mpsc;
use saikuro_exec::timeout;

use crate::shared::error::{Result, TransportError};
use crate::shared::host::{HostPipeFactory, HostPipeRecv, HostPipeSend, Role};
use crate::DEFAULT_CHANNEL_CAPACITY;

/// How long the active side waits for an accept reply before giving up.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// How often the active side re-announces its `connect` while waiting.
///
/// A browser `BroadcastChannel` is not a queue: a message posted before any
/// listener is attached is dropped. Re-posting on this cadence makes the
/// rendezvous immune to the listener not having finished wiring up its
/// `onmessage` handler when the connector first announces.
const CONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(150);

/// Control-message type tag carried in a JS object payload.
const MSG_ACCEPT: &str = "accept";
const MSG_CLOSE: &str = "close";
const MSG_CONNECT: &str = "connect";

/// Browser `BroadcastChannel` implementation of [`HostPipeFactory`].
pub struct BroadcastChannelPipe;

/// Shared, ref-counted private `BroadcastChannel` underlying one connection
/// half. The port stays open while either the send or receive half is alive;
/// it is closed, and the peer is told so, only once the last half drops.
struct WasmChannel {
    channel: BroadcastChannel,
    halves: Cell<usize>,
    closed: Cell<bool>,
}

impl WasmChannel {
    fn new(channel: BroadcastChannel) -> Arc<Self> {
        Arc::new(Self {
            channel,
            halves: Cell::new(2),
            closed: Cell::new(false),
        })
    }

    /// A send or receive half was dropped. Close the connection (posting a
    /// close control frame to the peer) once no halves remain.
    fn half_dropped(&self) {
        let remaining = self.halves.get() - 1;
        self.halves.set(remaining);
        if remaining == 0 {
            self.shutdown();
        }
    }

    /// Single close on the last half: tell the peer and release the port.
    fn shutdown(&self) {
        if self.closed.replace(true) {
            return;
        }
        let close = make_obj(&[("type", MSG_CLOSE)]);
        let _ = self.channel.post_message(&close);
        self.channel.close();
    }
}

/// Sending half of a [`BroadcastChannelPipe`] connection.
pub struct BroadcastChannelSend {
    shared: Arc<WasmChannel>,
}

/// Receiving half of a [`BroadcastChannelPipe`] connection.
pub struct BroadcastChannelRecv {
    shared: Arc<WasmChannel>,
    rx: mpsc::Receiver<Bytes>,
    _handler: SendWrapper<Closure<dyn FnMut(MessageEvent)>>,
}

impl Drop for BroadcastChannelSend {
    fn drop(&mut self) {
        self.shared.half_dropped();
    }
}

impl Drop for BroadcastChannelRecv {
    fn drop(&mut self) {
        self.shared.half_dropped();
    }
}

/// The browser `wasm` engine's `WasmHostTransport` concrete type.
pub type WasmHost =
    crate::shared::host::WasmHostTransport<BroadcastChannelSend, BroadcastChannelRecv>;

#[async_trait(?Send)]
impl HostPipeFactory for BroadcastChannelPipe {
    type Send = BroadcastChannelSend;
    type Recv = BroadcastChannelRecv;

    async fn open(channel: &str, role: Role) -> Result<(Self::Send, Self::Recv)> {
        match role {
            Role::Connect => open_connect(channel).await,
            Role::Accept => open_accept(channel).await,
        }
    }
}

#[async_trait(?Send)]
impl HostPipeSend for BroadcastChannelSend {
    async fn send(&mut self, frame: &[u8]) -> Result<()> {
        if self.shared.closed.get() {
            return Err(TransportError::ConnectionLost(
                "connection is closed".into(),
            ));
        }
        send_buffer(&self.shared.channel, frame)
    }

    async fn close(&mut self) -> Result<()> {
        self.shared.shutdown();
        Ok(())
    }
}

#[async_trait(?Send)]
impl HostPipeRecv for BroadcastChannelRecv {
    async fn recv(&mut self) -> Result<Option<Vec<u8>>> {
        match self.rx.recv().await {
            Some(bytes) => Ok(Some(bytes.to_vec())),
            None => Ok(None),
        }
    }
}

/// Active side: open a private channel, advertise connect, await accept.
async fn open_connect(channel: &str) -> Result<(BroadcastChannelSend, BroadcastChannelRecv)> {
    let conn_id = short_id()?;
    let private_name = format!("{}:{}", channel, conn_id);

    let private = BroadcastChannel::new(&private_name)
        .map_err(|e| TransportError::ConnectionLost(format!("{e:?}")))?;
    let shared = WasmChannel::new(private);

    let (data_tx, data_rx) = mpsc::channel::<Bytes>(DEFAULT_CHANNEL_CAPACITY);
    // Capacity-1 signal that an accept arrived. `mpsc` rather than `oneshot` so
    // the receiver can be re-polled across the re-announce loop below without
    // being consumed (a `oneshot` receiver can only be awaited once).
    let (accept_tx, mut accept_rx) = mpsc::channel::<()>(
        saikuro_exec::ChannelCapacity::try_from(1).expect("1 is a valid channel capacity"),
    );
    let data_slot = RefCell::new(Some(data_tx));

    let expected = conn_id.clone();
    let handler: SendWrapper<Closure<dyn FnMut(MessageEvent)>> =
        SendWrapper::new(Closure::new(move |event: MessageEvent| {
            let data = event.data();
            if let Some(t) = get_field(&data, "type") {
                match t.as_str() {
                    MSG_ACCEPT if get_field(&data, "id").as_deref() == Some(expected.as_str()) => {
                        let _ = accept_tx.try_send(());
                        return;
                    }
                    MSG_CLOSE => {
                        *data_slot.borrow_mut() = None;
                        return;
                    }
                    _ => {}
                }
            }
            if let Some(bytes) = extract_binary(&data) {
                if let Some(tx) = data_slot.borrow().as_ref() {
                    let _ = tx.try_send(Bytes::from(bytes));
                }
            }
        }));
    shared
        .channel
        .set_onmessage(Some((&*handler).as_ref().unchecked_ref()));

    // Announce `connect`, then keep re-announcing until accepted. Re-posting on
    // a fixed cadence converges once the listener is wired up, bounded by
    // `CONNECT_TIMEOUT`.
    let base = BroadcastChannel::new(channel)
        .map_err(|e| TransportError::ConnectionLost(format!("{e:?}")))?;
    let msg = make_obj(&[("type", MSG_CONNECT), ("id", &conn_id)]);
    let mut remaining = CONNECT_TIMEOUT;
    loop {
        base.post_message(&msg)
            .map_err(|e| TransportError::ConnectionLost(format!("{e:?}")))?;

        // `accept_rx` is an `mpsc` receiver borrowed mutably, so a timed-out
        // wait can be re-polled on the next loop iteration without consuming it.
        match timeout(CONNECT_RETRY_INTERVAL.min(remaining), accept_rx.recv()).await {
            Ok(Some(())) => {
                drop(base);
                let send = BroadcastChannelSend {
                    shared: shared.clone(),
                };
                let recv = BroadcastChannelRecv {
                    shared,
                    rx: data_rx,
                    _handler: handler,
                };
                return Ok((send, recv));
            }
            Ok(None) => {
                drop(base);
                return Err(TransportError::ConnectionLost(
                    "accept channel closed".into(),
                ));
            }
            Err(_) => {
                remaining = remaining.saturating_sub(CONNECT_RETRY_INTERVAL);
                if remaining.is_zero() {
                    drop(base);
                    return Err(TransportError::ConnectionLost("connect timeout".into()));
                }
            }
        }
    }
}

/// Passive side: listen on the base channel, answer each connect with an accept.
async fn open_accept(channel: &str) -> Result<(BroadcastChannelSend, BroadcastChannelRecv)> {
    let base = BroadcastChannel::new(channel)
        .map_err(|e| TransportError::ConnectionLost(format!("{e:?}")))?;

    let (conn_tx, mut conn_rx) = mpsc::channel::<String>(
        saikuro_exec::ChannelCapacity::try_from(32).expect("32 is a valid channel capacity"),
    );
    let base_handler: SendWrapper<Closure<dyn FnMut(MessageEvent)>> =
        SendWrapper::new(Closure::new({
            let conn_tx = conn_tx;
            move |event: MessageEvent| {
                let data = event.data();
                if get_field(&data, "type").as_deref() != Some(MSG_CONNECT) {
                    return;
                }
                if let Some(id) = get_field(&data, "id") {
                    let _ = conn_tx.try_send(id);
                }
            }
        }));
    base.set_onmessage(Some((&*base_handler).as_ref().unchecked_ref()));

    let conn_id = match conn_rx.recv().await {
        Some(id) => id,
        None => return Err(TransportError::ConnectionLost("base channel closed".into())),
    };
    drop(base_handler);
    drop(base);

    let private_name = format!("{}:{}", channel, conn_id);
    let private = BroadcastChannel::new(&private_name)
        .map_err(|e| TransportError::ConnectionLost(format!("{e:?}")))?;
    let shared = WasmChannel::new(private);

    let (data_tx, data_rx) = mpsc::channel::<Bytes>(DEFAULT_CHANNEL_CAPACITY);
    let data_slot = RefCell::new(Some(data_tx));
    let data_handler: Closure<dyn FnMut(MessageEvent)> =
        Closure::new(move |event: MessageEvent| {
            let data = event.data();
            if get_field(&data, "type").as_deref() == Some(MSG_CLOSE) {
                *data_slot.borrow_mut() = None;
                return;
            }
            if let Some(bytes) = extract_binary(&data) {
                if let Some(tx) = data_slot.borrow().as_ref() {
                    let _ = tx.try_send(Bytes::from(bytes));
                }
            }
        });
    shared
        .channel
        .set_onmessage(Some(data_handler.as_ref().unchecked_ref()));

    let msg = make_obj(&[("type", MSG_ACCEPT), ("id", &conn_id)]);
    shared
        .channel
        .post_message(&msg)
        .map_err(|e| TransportError::ConnectionLost(format!("{e:?}")))?;

    let send = BroadcastChannelSend {
        shared: shared.clone(),
    };
    let recv = BroadcastChannelRecv {
        shared,
        rx: data_rx,
        _handler: SendWrapper::new(data_handler),
    };
    Ok((send, recv))
}

/// Generate a 128-bit random hex connection identifier via the browser CSPRNG.
fn short_id() -> Result<String> {
    let crypto: Crypto = Reflect::get(&js_sys::global(), &"crypto".into())
        .map_err(|e| TransportError::ConnectionLost(format!("crypto API not found: {e:?}")))?
        .unchecked_into();
    let mut buf = [0u8; 16];
    crypto
        .get_random_values_with_u8_array(&mut buf)
        .map_err(|e| {
            TransportError::ConnectionLost(format!("crypto get_random_values failed: {e:?}"))
        })?;
    Ok(buf.iter().fold(String::with_capacity(32), |mut s, b| {
        let _ = write!(s, "{:02x}", b);
        s
    }))
}

/// Create a JS object literal from key-value pairs.
fn make_obj(pairs: &[(&str, &str)]) -> JsValue {
    let obj = js_sys::Object::new();
    for (k, v) in pairs {
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str(k), &JsValue::from_str(v));
    }
    JsValue::from(obj)
}

/// Try to extract a string field from a JS object-typed `JsValue`.
fn get_field(val: &JsValue, key: &str) -> Option<String> {
    js_sys::Reflect::get(val, &JsValue::from_str(key))
        .ok()
        .and_then(|v| v.as_string())
}

/// Pull the bytes out of a `BroadcastChannel` message payload.
fn extract_binary(data: &JsValue) -> Option<Vec<u8>> {
    if let Some(buf) = data.dyn_ref::<ArrayBuffer>() {
        Some(Uint8Array::new(buf).to_vec())
    } else if let Some(arr) = data.dyn_ref::<Uint8Array>() {
        Some(arr.to_vec())
    } else {
        None
    }
}

/// Post a binary frame as a freshly-allocated `ArrayBuffer`.
fn send_buffer(channel: &BroadcastChannel, frame: &[u8]) -> Result<()> {
    let len = frame.len() as u32;
    let buffer = ArrayBuffer::new(len);
    let dst = Uint8Array::new(&buffer);
    // Zero-copy view into WASM linear memory; consumed immediately in `set`.
    let src = unsafe { Uint8Array::view(frame) };
    dst.set(&JsValue::from(src), 0);
    channel
        .post_message(&JsValue::from(buffer))
        .map_err(|e| TransportError::SendFailed(format!("{e:?}")))
}
