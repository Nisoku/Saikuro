#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

//! Logging types and sinks for Saikuro.

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

#[cfg(any(
    all(feature = "native", any(feature = "no_std", feature = "wasm", feature = "embedded")),
    all(feature = "no_std", any(feature = "native", feature = "wasm", feature = "embedded")),
    all(feature = "wasm", any(feature = "native", feature = "no_std", feature = "embedded")),
    all(feature = "embedded", any(feature = "native", feature = "no_std", feature = "wasm"))
))]
compile_error!("exactly one engine must be enabled: native | no_std | wasm | embedded");

#[cfg(all(feature = "std", feature = "no_std"))]
compile_error!("the no_std engine cannot be combined with the std toolchain");

#[cfg(not(any(feature = "native", feature = "no_std", feature = "wasm", feature = "embedded")))]
compile_error!("exactly one engine must be selected: native | no_std | wasm | embedded");

mod shared;
pub use shared::*;

#[cfg(any(feature = "wasm", feature = "embedded", feature = "no_std"))]
mod base;
#[cfg(any(feature = "wasm", feature = "embedded", feature = "no_std"))]
pub use base::*;

#[cfg(feature = "native")]
mod native;
#[cfg(feature = "native")]
pub use native::*;

#[cfg(feature = "wasm")]
mod wasm;
#[cfg(feature = "wasm")]
pub use wasm::*;

#[cfg(feature = "embedded")]
mod embedded;
#[cfg(feature = "embedded")]
pub use embedded::*;
