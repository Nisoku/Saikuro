#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]

//! Invocation routing, provider registry, and stream-delivery state for the
//! Saikuro runtime.

extern crate alloc;

/// Provider registry and handle abstractions.
pub mod provider;
/// The core invocation router.
pub mod router;
/// Stream and channel delivery state.
pub mod stream_state;

pub use provider::{Provider, ProviderHandle, ProviderRegistry};
pub use router::{InvocationRouter, RouterConfig};
pub use stream_state::{ChannelState, StreamState, StreamStateStore};

//  Default log sink per engine.
/// The default [`LogSink`](saikuro_event::log::LogSink) for the native engine.
#[cfg(feature = "native")]
pub type DefaultRouterSink = saikuro_event::TracingSink;
/// The default [`LogSink`](saikuro_event::log::LogSink) for the wasm engine.
#[cfg(feature = "wasm")]
pub type DefaultRouterSink = saikuro_event::ConsoleSink;
/// The default [`LogSink`](saikuro_event::log::LogSink) for no_std/embedded engines.
#[cfg(any(feature = "no_std", feature = "embedded"))]
pub type DefaultRouterSink = saikuro_event::NullSink;

//  Compilation guard: exactly one engine backend must be selected.
#[cfg(not(any(
    feature = "native",
    feature = "no_std",
    feature = "wasm",
    feature = "embedded"
)))]
compile_error!(
    "saikuro-router: enable exactly one engine feature: native, no_std, wasm, or embedded"
);
