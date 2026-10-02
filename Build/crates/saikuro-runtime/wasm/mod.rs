use alloc::string::String;

#[cfg(feature = "asyncify")]
use core::cell::RefCell;

#[cfg(feature = "asyncify")]
use embassy_sync::blocking_mutex::CriticalSectionMutex;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::transport_adapter::HostPipeListener;
#[cfg(feature = "asyncify")]
use crate::RuntimeHandle;
use crate::SaikuroRuntime;
use saikuro_exec::watch;
use saikuro_transport::wasm::host_browser::BroadcastChannelPipe;

/// Start the runtime, listening for adapters that rendezvous on `channel`.
pub fn start_runtime(channel: String) {
    #[cfg(all(not(feature = "std"), not(feature = "embedded")))]
    crate::init_heap();

    saikuro_exec::run(async move {
        let runtime = SaikuroRuntime::builder().build().await;
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        runtime
            .serve(
                vec![HostPipeListener::<BroadcastChannelPipe>::new(channel)],
                shutdown_rx,
            )
            .await;
    });
}

/// Pump the executor once. Call from the browser event loop.
pub fn pump_executor() {
    saikuro_exec::pump();
}

/// JS entry point: start the runtime on `channel`.
#[wasm_bindgen]
pub fn start(channel: String) {
    start_runtime(channel);
}

/// JS entry point: pump the executor once from the browser event loop.
#[wasm_bindgen]
pub fn pump() {
    pump_executor();
}

/// Blocking JSON entry for hosts without JSPI.
///
/// Requires the module to have been built with `wasm-opt --asyncify` and
/// driven through the `callSync` wrapper `xtask` appends to the glue, since
/// `block_on` unwinds the wasm stack out to JS and back in.
#[cfg(feature = "asyncify")]
#[wasm_bindgen]
pub fn asyncify_call(request: &str) -> String {
    use saikuro_core::{CapabilitySet, Envelope};

    let envelope: Envelope = match serde_json::from_str(request) {
        Ok(envelope) => envelope,
        Err(error) => return malformed(&alloc::format!("{error}")),
    };

    let response = saikuro_exec::block_on(async move {
        let handle = asyncify_handle().await;
        handle.dispatch(envelope, &CapabilitySet::empty()).await
    });

    // Serialization of a ResponseEnvelope cannot fail; it is plain data.
    serde_json::to_string(&response).unwrap_or_else(|_| alloc::format!("{response:?}"))
}

/// A response for a request that never became an envelope, so there is no
/// invocation id to echo back.
#[cfg(feature = "asyncify")]
fn malformed(error: &str) -> String {
    let body = serde_json::json!({ "ok": false, "error": error });
    serde_json::to_string(&body).unwrap_or_else(|_| String::from("{\"ok\":false}"))
}

/// The process-wide runtime handle, built on first use.
#[cfg(feature = "asyncify")]
async fn asyncify_handle() -> RuntimeHandle {
    struct Slot(CriticalSectionMutex<RefCell<Option<RuntimeHandle>>>);

    // SAFETY: wasm is single threaded; see the module docs on the engine guard.
    unsafe impl Sync for Slot {}

    static SLOT: Slot = Slot(CriticalSectionMutex::new(RefCell::new(None)));

    let cached = SLOT.0.lock(|cell| cell.borrow().clone());
    if let Some(handle) = cached {
        return handle;
    }

    #[cfg(all(not(feature = "std"), not(feature = "embedded")))]
    crate::init_heap();

    let runtime = SaikuroRuntime::builder().build().await;
    let handle = runtime.handle();
    // Keep the runtime alive for the module's lifetime: dropping it would tear
    // down the registries that `dispatch` resolves against.
    core::mem::forget(runtime);
    SLOT.0
        .lock(|cell| *cell.borrow_mut() = Some(handle.clone()));
    handle
}
