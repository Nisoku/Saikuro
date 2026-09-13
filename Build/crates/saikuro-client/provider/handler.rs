//! Handler types: what an invokable function looks like.

use alloc::vec::Vec;

use crate::Value;

#[cfg(any(feature = "wasm", feature = "embedded", feature = "no_std"))]
use super::base::BoxedHandler;
#[cfg(feature = "native")]
use super::native::BoxedHandler;

/// Arguments passed to a registered handler function.
pub type HandlerArgs = Vec<Value>;

/// Options that can be supplied when registering a function.
#[derive(Debug, Clone, Default)]
pub struct RegisterOptions {
    /// Optional schema metadata describing the function.
    pub schema: Option<crate::FunctionSchema>,
}

/// Internal handler entry.
pub(super) struct HandlerEntry {
    pub(super) handler: BoxedHandler,
    pub(super) schema: Option<crate::FunctionSchema>,
}
