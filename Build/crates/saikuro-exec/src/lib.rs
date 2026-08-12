//! Lightweight execution facade used by Saikuro.
//!
//! Re-exports one backend implementation depending on enabled cargo features.
//! Supported backends: `tokio-runtime` (default), `wasm-runtime`, `embassy-runtime`.

#![cfg_attr(feature = "embassy-runtime", no_std)]

#[cfg(feature = "embassy-runtime")]
extern crate alloc;

mod capacity;
pub use capacity::{ChannelCapacity, InvalidChannelCapacity};

#[cfg(all(feature = "embassy-runtime", feature = "net"))]
mod embassy_net;

#[cfg(all(feature = "tokio-runtime", feature = "wasm-runtime"))]
compile_error!("Features `tokio-runtime` and `wasm-runtime` are mutually exclusive.");

#[cfg(all(feature = "tokio-runtime", feature = "embassy-runtime"))]
compile_error!("Features `tokio-runtime` and `embassy-runtime` are mutually exclusive.");

#[cfg(all(feature = "wasm-runtime", feature = "embassy-runtime"))]
compile_error!("Features `wasm-runtime` and `embassy-runtime` are mutually exclusive.");

#[cfg(not(any(
    feature = "tokio-runtime",
    feature = "wasm-runtime",
    feature = "embassy-runtime"
)))]
compile_error!(
    "saikuro-exec: no runtime backend selected. \
     Enable one of `tokio-runtime`, `wasm-runtime`, or `embassy-runtime`."
);

#[cfg(any(feature = "tokio-runtime", feature = "wasm-runtime"))]
pub use tokio as _tokio;

#[cfg(feature = "tokio-runtime")]
mod tokio_backend;
#[cfg(feature = "tokio-runtime")]
pub use tokio_backend::*;

#[cfg(feature = "wasm-runtime")]
mod wasm_backend;
#[cfg(feature = "wasm-runtime")]
pub use wasm_backend::*;

#[cfg(feature = "embassy-runtime")]
mod embassy_backend;
#[cfg(feature = "embassy-runtime")]
pub use embassy_backend::*;

#[cfg(feature = "embassy-runtime")]
pub use futures as _futures;

/// Branch on the first future to complete.
///
/// The tokio and wasm backends delegate to `tokio::select!` and accept its
/// full syntax.  The embassy backend only supports `pattern = future => { ... }`
/// branches (see `select_impl!`); it rejects `else`, `biased;`, guards, and
/// expression handlers.  Cross-backend code must stay within the shared subset
/// so it compiles on every backend.
#[macro_export]
macro_rules! select {
    ($($tt:tt)*) => {
        $crate::select_impl! { $($tt)* }
    };
}

#[doc(hidden)]
#[cfg(any(feature = "tokio-runtime", feature = "wasm-runtime"))]
#[macro_export]
macro_rules! select_impl {
    ($($tt:tt)*) => {
        $crate::_tokio::select! { $($tt)* }
    };
}

/// Embassy-compatible `select!`.
///
/// Delegates to `futures::select_biased!`, which requires every branch future
/// to implement `FusedFuture`.  Each branch is fused at the facade boundary so
/// call sites pass plain futures (`listener.accept()`, `forward_rx.recv()`,
/// and friends).  `select_biased!` is used rather than `futures::select!`
/// because the latter is gated behind the `std` feature and cannot resolve on
/// `no_std` MCU targets.
#[doc(hidden)]
#[cfg(feature = "embassy-runtime")]
#[macro_export]
macro_rules! select_impl {
    (
        $(
            $pattern:pat = $fut:expr => $handler:block $(,)?
        )+
    ) => {
        $crate::_futures::select_biased! {
            $(
                $pattern = $crate::fuse_select($fut) => $handler ,
            )+
        }
    };
}
