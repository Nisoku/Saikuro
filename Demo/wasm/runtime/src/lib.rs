use wasm_bindgen::prelude::*;

/// JS entry point: start the runtime, listening for adapters that
/// rendezvous on `channel`. The accept loop and peer wiring live in
/// `saikuro_runtime::wasm::start_runtime`.
#[wasm_bindgen]
pub async fn start_runtime(channel: String) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    saikuro_runtime::wasm::start_runtime(channel);
    Ok(())
}