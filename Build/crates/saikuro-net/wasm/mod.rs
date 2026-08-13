//! Browser (`wasm32-unknown-unknown`) engine.
//!
//! Networking on the browser has no TCP/UDP socket API exposed to Rust, so
//! `net`/`io` are not provided here. A WASI (preview1 or preview2 component)
//! build should select the `no_std` engine instead.
