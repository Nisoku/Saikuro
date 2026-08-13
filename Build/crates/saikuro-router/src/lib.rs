//! Saikuro Router
//!
//! This crate owns the invocation router and provider registry.  It maps
//! namespace names to provider handles and dispatches incoming envelopes.
//!
//! The crate is `no_std` + `alloc`

#![no_std]

#[macro_use]
extern crate alloc;

pub mod error;
pub mod provider;
pub mod router;
pub mod stream_state;

pub use error::RouterError;
pub use provider::{Provider, ProviderHandle, ProviderRegistry};
pub use router::{InvocationRouter, RouterConfig};
pub use stream_state::{ChannelState, StreamState, StreamStateStore};
