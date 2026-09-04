#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]

//! Schema, capability, and invocation-validation types for the Saikuro runtime.

#[macro_use]
extern crate alloc;

/// Ergonomic schema construction (FunctionSchema, NamespaceSchema, build_schema).
pub mod builder;
/// Capability enforcement engine.
pub mod capability;
/// Capability enforcement engine (re-exported module path).
pub use capability::engine as capability_engine;
/// Schema registry and namespace management.
pub mod registry;
/// Invocation validator.
pub mod validator;

pub use capability::engine::CapabilityEngine;
pub use builder::{FunctionSchema, NamespaceSchema, ArgDescriptor, build_schema};
pub use registry::{NamespaceRegistration, SchemaRegistry};
pub use validator::{InvocationValidator, ValidationReport};

// Compilation guard: exactly one engine backend must be selected.
#[cfg(not(any(
    feature = "native",
    feature = "no_std",
    feature = "wasm",
    feature = "embedded"
)))]
compile_error!(
    "saikuro-schema: enable exactly one engine feature: native, no_std, wasm, or embedded"
);
