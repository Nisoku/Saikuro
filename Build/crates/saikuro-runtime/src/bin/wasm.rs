#![cfg(feature = "wasm")]

use saikuro_exec::watch;
use saikuro_runtime::transport_adapter::HostPipeListener;
use saikuro_runtime::SaikuroRuntime;
use saikuro_transport::wasm::host_browser::BroadcastChannelPipe;

/// Start the runtime, listening for adapters that rendezvous on `channel`.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn start(channel: String) {
    let runtime = SaikuroRuntime::builder().build();
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    saikuro_exec::spawn(async move {
        runtime
            .serve(
                vec![HostPipeListener::<BroadcastChannelPipe>::new(channel)],
                shutdown_rx,
            )
            .await;
    });
}

/// Pump the executor once. Call from the browser event loop.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn pump() {
    saikuro_exec::pump();
}
