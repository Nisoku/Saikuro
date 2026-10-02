//! WASI preview 2 math example (`wasm32-wasip2`).
//!
//! Run with:
//!   cargo build -p math-wasi-preview2 --target wasm32-wasip2
//!   cargo run   -p math-wasi-preview2 --target wasm32-wasip2

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
            "math-wasi-preview2 only supports --transport memory".into(),
        ));
    }
    run_in_memory().await
}
