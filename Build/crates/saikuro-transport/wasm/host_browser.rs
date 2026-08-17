use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use async_trait::async_trait;
use bytes::Bytes;
use core::fmt::Write;
use core::time::Duration;
use js_sys::{ArrayBuffer, Reflect, Uint8Array};
use send_wrapper::SendWrapper;
use tracing::trace;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use web_sys::{BroadcastChannel, Crypto, MessageEvent};

use saikuro_exec::mpsc;
use saikuro_exec::oneshot;
use saikuro_exec::timeout;

use crate::shared::error::{Result, TransportError};
use crate::shared::host::{HostPipeFactory, HostPipeRecv, HostPipeSend, Role};
use crate::DEFAULT_CHANNEL_CAPACITY;

/// How long the active side waits for an accept reply before giving up.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Browser `BroadcastChannel` implementation of [`HostPipeFactory`].
pub struct BroadcastChannelPipe;

/// Sending half of a [`BroadcastChannelPipe`] connection.
pub struct BroadcastChannelSend {
    channel: SendWrapper<BroadcastChannel>,
}

/// Receiving half of a [`BroadcastChannelPipe`] connection.
pub struct BroadcastChannelRecv {
    channel: SendWrapper<BroadcastChannel>,
    rx: mpsc::Receiver<Bytes>,
    _handler: SendWrapper<Closure<dyn FnMut(MessageEvent)>>,
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
        trace!(bytes = frame.len(), "wasm-host send");
        send_buffer(&self.channel, frame)
    }
}

#[async_trait(?Send)]
impl HostPipeRecv for BroadcastChannelRecv {
    async fn recv(&mut self) -> Result<Option<Vec<u8>>> {
        match self.rx.recv().await {
            Some(bytes) => {
                trace!(bytes = bytes.len(), "wasm-host recv");
                Ok(Some(bytes.to_vec()))
            }
            None => {
                trace!("wasm-host channel closed");
                Ok(None)
            }
        }
    }
}

impl Drop for BroadcastChannelRecv {
    fn drop(&mut self) {
        self.channel.set_onmessage(None);
        self.channel.close();
    }
}

/// Active side: open a private channel, advertise connect, await accept.
async fn open_connect(channel: &str) -> Result<(BroadcastChannelSend, BroadcastChannelRecv)> {
    let conn_id = short_id()?;
    let private_name = format!("{}:{}", channel, conn_id);

    let private = BroadcastChannel::new(&private_name)
        .map_err(|e| TransportError::ConnectionLost(format!("{e:?}")))?;

    let (data_tx, data_rx) = mpsc::channel::<Bytes>(DEFAULT_CHANNEL_CAPACITY);
    let (accept_tx, accept_rx) = oneshot::channel::<()>();

    let handler: SendWrapper<Closure<dyn FnMut(MessageEvent)>> = SendWrapper::new(Closure::new({
        let data_tx = data_tx;
        let accept_tx = accept_tx;
        let expected = conn_id.clone();
        move |event: MessageEvent| {
            let data = event.data();
            // A handshake accept is a JS object, not a binary frame.
            if let Some(t) = get_field(&data, "type") {
                if t == "accept" && get_field(&data, "id").as_deref() == Some(expected.as_str()) {
                    let _ = accept_tx.try_send(());
                    return;
                }
            }
            if let Some(bytes) = extract_binary(&data) {
                let _ = data_tx.try_send(Bytes::from(bytes));
            }
        }
    }));
    private.set_onmessage(Some((&*handler).as_ref().unchecked_ref()));

    let base = BroadcastChannel::new(channel)
        .map_err(|e| TransportError::ConnectionLost(format!("{e:?}")))?;
    let msg = make_obj(&[("type", "connect"), ("id", &conn_id)]);
    base.post_message(&msg)
        .map_err(|e| TransportError::ConnectionLost(format!("{e:?}")))?;
    drop(base);

    match timeout(CONNECT_TIMEOUT, accept_rx.recv()).await {
        Ok(Ok(())) => {
            let send = BroadcastChannelSend {
                channel: SendWrapper::new(private.clone()),
            };
            let recv = BroadcastChannelRecv {
                channel: SendWrapper::new(private),
                rx: data_rx,
                _handler: handler,
            };
            Ok((send, recv))
        }
        Ok(Err(_)) => Err(TransportError::ConnectionLost(
            "accept channel closed".into(),
        )),
        Err(_) => Err(TransportError::ConnectionLost("connect timeout".into())),
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
                if get_field(&data, "type").as_deref() != Some("connect") {
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

    let (data_tx, data_rx) = mpsc::channel::<Bytes>(DEFAULT_CHANNEL_CAPACITY);
    let data_handler: Closure<dyn FnMut(MessageEvent)> = Closure::new({
        let data_tx = data_tx;
        move |event: MessageEvent| {
            if let Some(bytes) = extract_binary(&event.data()) {
                let _ = data_tx.try_send(Bytes::from(bytes));
            }
        }
    });
    private.set_onmessage(Some(data_handler.as_ref().unchecked_ref()));

    let msg = make_obj(&[("type", "accept"), ("id", &conn_id)]);
    private
        .post_message(&msg)
        .map_err(|e| TransportError::ConnectionLost(format!("{e:?}")))?;

    let send = BroadcastChannelSend {
        channel: SendWrapper::new(private.clone()),
    };
    let recv = BroadcastChannelRecv {
        channel: SendWrapper::new(private),
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
