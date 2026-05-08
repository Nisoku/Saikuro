//! Lightweight execution facade used by Saikuro.
//!
//! This file re-exports one of the backend implementations depending on
//! enabled cargo features. Preferred backends are `tokio-runtime` (default),
//! `wasm-runtime`, and `embassy-runtime`.

#[cfg(all(
    feature = "tokio-runtime",
    any(feature = "wasm-runtime", feature = "embassy-runtime")
))]
compile_error!("saikuro-exec: multiple backends selected. Enable only ONE of `tokio-runtime`, `wasm-runtime`, or `embassy-runtime`.");

#[cfg(all(
    feature = "wasm-runtime",
    any(feature = "tokio-runtime", feature = "embassy-runtime")
))]
compile_error!("saikuro-exec: multiple backends selected. Enable only ONE of `tokio-runtime`, `wasm-runtime`, or `embassy-runtime`.");

#[cfg(all(
    feature = "embassy-runtime",
    any(feature = "tokio-runtime", feature = "wasm-runtime")
))]
compile_error!("saikuro-exec: multiple backends selected. Enable only ONE of `tokio-runtime`, `wasm-runtime`, or `embassy-runtime`.");

#[cfg(feature = "tokio-runtime")]
mod tokio_backend;
#[cfg(feature = "tokio-runtime")]
pub use tokio as _tokio;
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

#[cfg(not(any(
    feature = "tokio-runtime",
    feature = "wasm-runtime",
    feature = "embassy-runtime"
)))]
compile_error!("saikuro-exec: no runtime backend selected. Enable one of `tokio-runtime`, `wasm-runtime`, or `embassy-runtime`.");

#[macro_export]
macro_rules! select {
	($($tt:tt)*) => {
		$crate::select_impl! { $($tt)* }
	};
}

#[doc(hidden)]
#[macro_export]
#[cfg(feature = "tokio-runtime")]
macro_rules! select_impl {
	($($tt:tt)*) => {
		$crate::_tokio::select! { $($tt)* }
	};
}

#[doc(hidden)]
#[macro_export]
#[cfg(feature = "wasm-runtime")]
macro_rules! select_impl {
	($($tt:tt)*) => {
		futures::select! { $($tt)* }
	};
}

#[doc(hidden)]
#[macro_export]
#[cfg(feature = "embassy-runtime")]
macro_rules! select_impl {
    ($($tt:tt)*) => {
        compile_error!("embassy-runtime select! is not implemented yet");
    };
}
