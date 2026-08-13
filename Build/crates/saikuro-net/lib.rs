#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

//! Networking and IO facade for Saikuro.

#[cfg(all(
    feature = "native",
    any(feature = "no_std", feature = "embedded", feature = "wasm")
))]
compile_error!("only one of native/no_std/embedded/wasm may be enabled");
#[cfg(all(feature = "no_std", any(feature = "embedded", feature = "wasm")))]
compile_error!("only one of native/no_std/embedded/wasm may be enabled");
#[cfg(all(feature = "embedded", feature = "wasm"))]
compile_error!("only one of native/no_std/embedded/wasm may be enabled");
#[cfg(not(any(
    feature = "native",
    feature = "no_std",
    feature = "embedded",
    feature = "wasm"
)))]
compile_error!("exactly one of native/no_std/embedded/wasm must be enabled");

#[cfg(feature = "native")]
pub mod native;
#[cfg(feature = "no_std")]
pub mod no_std;
#[cfg(feature = "embedded")]
pub mod embedded;
#[cfg(feature = "wasm")]
pub mod wasm;
