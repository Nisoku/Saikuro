//! Saikuro client library.
//!
//! Provides [`Provider`] and [`Client`]: the two main building blocks for
//! writing Rust services that connect to a Saikuro runtime.

#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

// Engine selection guard
#[cfg(all(
    feature = "native",
    any(feature = "wasm", feature = "embedded", feature = "no_std")
))]
compile_error!(
    "saikuro-client: only one engine feature (native/wasm/embedded/no_std) may be enabled"
);

#[cfg(all(feature = "wasm", any(feature = "embedded", feature = "no_std")))]
compile_error!(
    "saikuro-client: only one engine feature (native/wasm/embedded/no_std) may be enabled"
);

#[cfg(all(feature = "embedded", feature = "no_std"))]
compile_error!(
    "saikuro-client: only one engine feature (native/wasm/embedded/no_std) may be enabled"
);

#[cfg(not(any(
    feature = "native",
    feature = "wasm",
    feature = "embedded",
    feature = "no_std"
)))]
compile_error!("saikuro-client: an engine feature (native/wasm/embedded/no_std) must be enabled");

// On WASI `std` is the libc base and `no_std` selects the engine.
#[cfg(all(feature = "std", feature = "no_std", not(target_os = "wasi")))]
compile_error!("saikuro-client: the `std` toolchain flag is incompatible with the `no_std` engine");

pub mod shared;

pub mod client;
pub mod provider;

pub use client::Client;
pub use provider::{HandlerArgs, Provider, RegisterOptions};
pub use saikuro_core::schema::{PrimitiveType, TypeDescriptor, Visibility};
pub use saikuro_schema::builder::{build_schema, ArgDescriptor, FunctionSchema, NamespaceSchema};
pub use saikuro_transport::{connect, AdapterTransport, MemoryAdapterTransport};
pub use shared::{ClientOptions, SaikuroChannel, SaikuroStream};

/// The value type used throughout the Saikuro Rust adapter.
pub type Value = serde_json::Value;

pub use saikuro_event::{Result, SaikuroError as Error};
