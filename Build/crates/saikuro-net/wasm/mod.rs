//! Browser (`wasm32-unknown-unknown`) engine.
//!
//! Networking on the browser has no TCP/UDP socket API, so `net`/`io` are not
//! provided here.
//!
//! A WASI (preview2 component) build should select the
//! `no_std` engine instead.

pub mod net;
pub mod io;
