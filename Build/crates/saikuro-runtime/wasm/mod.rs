use alloc::string::String;

use wasm_bindgen::prelude::wasm_bindgen;

use crate::transport_adapter::HostPipeListener;
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
