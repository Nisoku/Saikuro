//! Saikuro Router
//!
//! This crate owns the invocation router and provider registry.  It maps
//! namespace names to provider handles and dispatches incoming envelopes.

pub mod error;
pub mod provider;
pub mod router;
pub mod stream_state;

pub use error::RouterError;
pub use provider::{Provider, ProviderHandle, ProviderRegistry};
pub use router::{tracing_log_sink, InvocationRouter, RouterConfig};
pub use stream_state::{ChannelState, StreamState, StreamStateStore};
