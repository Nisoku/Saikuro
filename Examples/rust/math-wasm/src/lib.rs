//! Browser wasm build of the math example (`wasm32-unknown-unknown`).
//!
//! Build and serve:
//!   cd Examples/rust/math-wasm
//!   wasm-pack build --target web --out-dir ../../browser/pkg
//!   npx serve Examples/browser
//!
//! `Examples/browser/index.html` runs [`run_memory`] on load and exposes
//! buttons for the in-realm provider and client.

use core::cell::RefCell;

use saikuro::transport::{BroadcastChannelPipe, LocalTransport, LocalTransportListener};
use saikuro::Client;
use wasm_bindgen::prelude::*;

// The page's output callback, if one has been installed.
thread_local! {
    static OUTPUT: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
}

/// Route the shared demo's output to a JS callback.
///
/// Pass `null` to drop it. With no callback the demo still runs and writes to
/// the browser console.
#[wasm_bindgen]
pub fn set_output(callback: Option<js_sys::Function>) {
    OUTPUT.with(|slot| *slot.borrow_mut() = callback);
}

/// Hand one demo line to the page callback, or to the console without one.
fn emit(line: &str) {
    OUTPUT.with(|slot| {
        if let Some(callback) = slot.borrow().as_ref() {
            // A throwing page callback must not take the module down.
            let _ = callback.call1(&JsValue::NULL, &JsValue::from_str(line));
        } else {
            web_sys::console::log_1(&JsValue::from_str(line));
        }
    });
}

/// Point `math_core`'s log sink at [`set_output`], falling back to the console.
fn use_output_sink() {
    math_core::set_log_sink(Some(emit));
}

/// Convert any Saikuro error into the `JsValue` wasm-bindgen surfaces to JS.
fn to_js<E: core::fmt::Display>(e: E) -> JsValue {
    JsValue::from_str(&format!("{e}"))
}

/// Run the shared math demo with the provider and client both in this page.
#[wasm_bindgen]
pub async fn run_memory() -> Result<(), JsValue> {
    use_output_sink();
    math_core::run_in_memory().await.map_err(to_js)
}

/// Serve the shared math provider on a `BroadcastChannel` rendezvous.
#[wasm_bindgen]
pub async fn run_wasm_host_provider(channel: String) -> Result<(), JsValue> {
    use saikuro::from_halves;
    use saikuro::transport::WasmHostListener;

    use_output_sink();
    math_core::demo_log!("transport: wasm-host (providing on channel {channel:?})");

    // Only the provider accepts.
    let mut listener = WasmHostListener::<BroadcastChannelPipe>::new(&channel);
    let transport = listener
        .accept()
        .await
        .map_err(to_js)?
        .ok_or_else(|| JsValue::from_str("listener closed before a client connected"))?;

    let (sender, receiver) = transport.split();
    math_core::math_provider()
        .serve_on(from_halves(sender, receiver))
        .await
        .map_err(to_js)
}

/// Run the shared client demo against a provider in another same-origin
/// context, over a `BroadcastChannel` rendezvous on `channel`.
#[wasm_bindgen]
pub async fn run_wasm_host_client(channel: String) -> Result<(), JsValue> {
    use_output_sink();
    math_core::demo_log!("transport: wasm-host (dialling channel {channel:?})");
    let client = Client::connect(format!("wasm-host://{channel}"))
        .await
        .map_err(to_js)?;
    math_core::run_demo(client).await.map_err(to_js)
}

/// Run the shared client demo against a WebSocket provider at `url`.
#[wasm_bindgen]
pub async fn run_ws_client(url: String) -> Result<(), JsValue> {
    use_output_sink();
    math_core::demo_log!("transport: ws (client dialling {url})");
    let client = Client::connect(url).await.map_err(to_js)?;
    math_core::run_demo(client).await.map_err(to_js)
}
