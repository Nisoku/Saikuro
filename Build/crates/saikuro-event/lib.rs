#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

//! Unified error taxonomy, structured logging, and dynamically-typed value
//! types for Saikuro.
#[macro_use]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

#[cfg(any(
    all(feature = "native", feature = "no_std"),
    all(feature = "native", feature = "wasm"),
    all(feature = "native", feature = "embedded"),
    all(feature = "no_std", feature = "wasm"),
    all(feature = "no_std", feature = "embedded"),
    all(feature = "wasm", feature = "embedded"),
))]
compile_error!("saikuro-event: enable exactly one engine (native / no_std / wasm / embedded)");

// On WASI `std` is the libc base and `no_std` selects the engine.
#[cfg(all(feature = "no_std", feature = "std", not(target_os = "wasi")))]
compile_error!("saikuro-event: the no_std engine cannot be combined with the std toolchain");

/// Dynamically-typed value types used across the Saikuro wire protocol.
pub mod value;
pub use value::*;

mod core_events;
pub use core_events::*;

/// Structured logging primitives and [`LogSink`](log::sink::LogSink) implementations.
pub mod log;
pub use log::*;

#[cfg(all(feature = "native", feature = "tracing"))]
pub use log::tracing::TracingSink;

#[cfg(feature = "console")]
pub use log::console::ConsoleSink;
