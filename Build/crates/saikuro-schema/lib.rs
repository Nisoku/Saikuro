#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]

extern crate alloc;

pub mod engine;
pub mod registry;
pub mod validator;

pub use engine::CapabilityEngine;
pub use registry::{ NamespaceRegistration, SchemaRegistry };
pub use validator::{ InvocationValidator, ValidationReport };

// Compilation guard: exactly one engine backend must be selected.
#[cfg(not(any(feature = "native", feature = "no_std", feature = "wasm", feature = "embedded")))]
compile_error!(
    "saikuro-schema: enable exactly one engine feature: native, no_std, wasm, or embedded"
);
