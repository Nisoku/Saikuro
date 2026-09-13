//! Engine-specific runtime transport-trait bridges.
#[cfg(feature = "native")]
mod native;
#[cfg(not(feature = "native"))]
mod nonnative;

#[cfg(feature = "native")]
pub use native::*;
#[cfg(not(feature = "native"))]
pub use nonnative::*;
