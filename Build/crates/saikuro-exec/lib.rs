//! Saikuro execution and concurrency facade.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

// Exactly one engine must be selected
#[cfg(any(
    all(
        feature = "native",
        any(feature = "no_std", feature = "wasm", feature = "embedded")
    ),
    all(
        feature = "no_std",
        any(feature = "native", feature = "wasm", feature = "embedded")
    ),
    all(
        feature = "wasm",
        any(feature = "native", feature = "no_std", feature = "embedded")
    ),
    all(
        feature = "embedded",
        any(feature = "native", feature = "no_std", feature = "wasm")
    )
))]
compile_error!("exactly one engine must be enabled: native | no_std | wasm | embedded");

#[cfg(all(feature = "std", feature = "no_std"))]
compile_error!("the no_std engine cannot be combined with the std toolchain");

mod shared;
pub use shared::JoinError;
pub use shared::{ChannelCapacity, InvalidChannelCapacity};

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

#[cfg(feature = "no_std")]
mod no_std;
#[cfg(feature = "no_std")]
pub use no_std::*;

#[cfg(feature = "embedded")]
mod embedded;
#[cfg(feature = "embedded")]
pub use embedded::*;

#[cfg(not(feature = "native"))]
pub use futures as _futures;
#[cfg(feature = "native")]
pub use tokio as _tokio;

#[macro_export]
macro_rules! select {
    ($($tt:tt)*) => {
        $crate::select_impl! { $($tt)* }
    };
}

#[doc(hidden)]
#[cfg(feature = "native")]
#[macro_export]
macro_rules! select_impl {
    ($($tt:tt)*) => {
        $crate::_tokio::select! { $($tt)* }
    };
}

#[doc(hidden)]
#[cfg(not(feature = "native"))]
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
