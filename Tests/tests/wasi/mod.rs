//! WASI-targeted tests (`wasm32-wasip1` / `wasm32-wasip2`).

use saikuro_tests::TestSuite;

#[cfg(feature = "ws-wasi")]
mod websocket;

#[cfg(feature = "wasi-preview2")]
mod tcp;

pub fn register(suite: &mut TestSuite) {
    #[cfg(feature = "ws-wasi")]
    websocket::register(suite);
    #[cfg(feature = "wasi-preview2")]
    tcp::register(suite);
}
