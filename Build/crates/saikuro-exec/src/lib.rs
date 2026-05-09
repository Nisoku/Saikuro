//! Lightweight execution facade used by Saikuro.
//!
//! This file re-exports one of the backend implementations depending on
//! enabled cargo features. Preferred backends are `tokio-runtime` (default),
//! `wasm-runtime`, and `embassy-runtime`.

#[cfg(all(feature = "tokio-runtime", feature = "wasm-runtime"))]
compile_error!("Feature `tokio-runtime` and `wasm-runtime` are mutually exclusive.");

#[cfg(all(feature = "tokio-runtime", feature = "embassy-runtime"))]
compile_error!("Feature `tokio-runtime` and `embassy-runtime` are mutually exclusive.");

#[cfg(all(feature = "wasm-runtime", feature = "embassy-runtime"))]
compile_error!("Feature `wasm-runtime` and `embassy-runtime` are mutually exclusive.");

#[cfg(not(any(
    feature = "tokio-runtime",
    feature = "wasm-runtime",
    feature = "embassy-runtime"
)))]
compile_error!("saikuro-exec: No runtime backend selected. Enable one of `tokio-runtime`, `wasm-runtime`, or `embassy-runtime`.");

#[cfg(feature = "tokio-runtime")]
mod tokio_backend;
#[cfg(feature = "tokio-runtime")]
pub use {tokio as _tokio, tokio_backend::*};

#[cfg(feature = "wasm-runtime")]
mod wasm_backend;
#[cfg(feature = "wasm-runtime")]
pub use wasm_backend::*;

#[cfg(feature = "embassy-runtime")]
mod embassy_backend;
#[cfg(feature = "embassy-runtime")]
pub use embassy_backend::*;

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
    (
        $( $pat:pat = $fut:expr => $body:block )+
    ) => {{
        // Custom poll-and-yield select for WASM.
        //
        // Each branch's future is polled once inside a block scope. If the
        // future yields `Poll::Ready` and the pattern matches, the handler
        // runs. The block scope ensures the future (and its borrows on `self`)
        // is dropped **before** the handler body executes.
        //
        // If no branch is ready, the macro yields to the executor and returns
        // `Poll::Pending` so the containing async fn is re-polled.
        let __waker = ::futures::task::noop_waker();
        let mut __cx = ::core::task::Context::from_waker(&__waker);
        let mut __selected = false;
        $(
            if !__selected {
                let __poll = {
                    let mut __fut = $fut;
                    ::futures::pin_mut!(__fut);
                    ::core::future::Future::poll(__fut.as_mut(), &mut __cx)
                };
                if let ::core::task::Poll::Ready(__val) = __poll {
                    if let $pat = __val {
                        __selected = true;
                        $body
                    }
                }
            }
        )+
        if !__selected {
            $crate::yield_now().await;
        }
    }};
}

#[doc(hidden)]
#[macro_export]
#[cfg(feature = "embassy-runtime")]
macro_rules! select_impl {
    ($($tt:tt)*) => {
        compile_error!("embassy-runtime select! is not implemented yet");
    };
}
