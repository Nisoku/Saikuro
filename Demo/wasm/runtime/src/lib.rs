use std::sync::atomic::{AtomicUsize, Ordering};

use once_cell::sync::OnceCell;
use wasm_bindgen::prelude::*;

use saikuro_core::capability::CapabilitySet;
use saikuro_runtime::SaikuroRuntime;
use saikuro_transport::wasm_host::WasmHostListener;
use saikuro_transport::traits::TransportListener;

static HANDLE: OnceCell<saikuro_runtime::RuntimeHandle> = OnceCell::new();
static PEER_COUNTER: AtomicUsize = AtomicUsize::new(1);

#[wasm_bindgen]
pub async fn start_runtime(channel: String) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    if HANDLE.get().is_some() {
        return Ok(());
    }

    let runtime = SaikuroRuntime::builder().build();
    let handle = runtime.handle();
    HANDLE.set(handle.clone()).map_err(|_| JsValue::from_str("runtime already started"))?;

    let mut listener = WasmHostListener::new(channel)
        .map_err(|e| JsValue::from_str(&format!("listener error: {e}")))?;

    saikuro_exec::spawn(async move {
        loop {
            match listener.accept().await {
                Ok(Some(transport)) => {
                    let id = PEER_COUNTER.fetch_add(1, Ordering::SeqCst);
                    let peer_id = format!("wasm-peer-{id}");
                    handle.accept_transport(transport, peer_id, CapabilitySet::default());
                }
                Ok(None) => break,
                Err(_) => {
                    // Non-fatal accept error: continue rather than
                    // permanently stopping the accept loop.
                }
            }
        }
    });

    Ok(())
}
