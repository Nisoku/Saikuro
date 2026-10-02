//! WASI preview 1 math example (`wasm32-wasip1`).
//!
//! The provider, the schema, and the client demo come from `math-core`, shared
//! with the native, preview 2, and browser examples. Only the transport wiring
//! differs: in-process MPSC channels instead of an OS socket.
//!
//! Run with:
//!   cargo build -p math-wasi-preview1 --target wasm32-wasip1
//!   cargo run   -p math-wasi-preview1 --target wasm32-wasip1

use math_core::{run_in_memory, Options, TransportChoice};
use saikuro::{Error, Result};

fn main() -> Result<()> {
    math_core::install_wasi_runtime_support();
    saikuro_exec::block_on(async_main())
}

async fn async_main() -> Result<()> {
    let options = Options::parse(std::env::args().skip(1))?;
    // TODO: This target has no socket backend
    if options.transport != TransportChoice::Memory {
        return Err(Error::ProviderError(
            "math-wasi-preview1 only supports --transport memory".into(),
        ));
    }
    run_in_memory().await
}
