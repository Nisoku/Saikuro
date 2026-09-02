//! Tests that exercise the embedded engines (Cortex-M / RISC-V QEMU).
//!
//! Each QEMU bin includes this module via `#[path = "../embedded/mod.rs"]`
//! and drives it with the shared `TestSuite`. Individual subtrees opt into
//! the engine features the bin enables.

pub mod storage;
pub mod transport;