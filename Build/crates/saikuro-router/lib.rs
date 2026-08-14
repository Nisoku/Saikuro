#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]

extern crate alloc;

pub mod error;
pub mod provider;
pub mod router;
pub mod stream_state;

pub use error::RouterError;
pub use provider::{Provider, ProviderHandle, ProviderRegistry};
pub use router::{InvocationRouter, RouterConfig};
pub use stream_state::{ChannelState, StreamState, StreamStateStore};

//  Default log sink per engine.
#[cfg(feature = "native")]
pub type DefaultRouterSink = saikuro_log::TracingSink;
#[cfg(feature = "wasm")]
pub type DefaultRouterSink = saikuro_log::ConsoleSink;
#[cfg(any(feature = "no_std", feature = "embedded"))]
pub type DefaultRouterSink = saikuro_log::NullSink;

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
