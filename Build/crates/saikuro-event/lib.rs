#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

//! Unified error taxonomy, structured logging, and dynamically-typed value
//! types for Saikuro.
#[cfg(not(feature = "std"))]
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

#[cfg(all(feature = "no_std", feature = "std"))]
compile_error!("saikuro-event: the no_std engine cannot be combined with the std toolchain");

mod value;
pub use value::*;

mod core_events;
pub use core_events::*;

pub mod log;
pub use log::*;
