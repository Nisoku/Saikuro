//! Saikuro Rust adapter.
//!
//! A re-export facade over all Saikuro crates.
//!
//! For testing without a live runtime use [`MemoryAdapterTransport`].

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

// Client/Provider: named top-level types so `transport`/`schema`/`exec`
// module names are reserved for the crates below.
pub use saikuro_client::{
    build_schema, connect, AdapterTransport, ArgDescriptor, Client, ClientOptions, Error,
    FunctionSchema, HandlerArgs, MemoryAdapterTransport, NamespaceSchema, PrimitiveType, Provider,
    RegisterOptions, Result, SaikuroChannel, SaikuroStream, TypeDescriptor, Value, Visibility,
};

pub mod core;
pub mod event;
pub mod exec;
#[cfg(feature = "net")]
pub mod net;
pub mod random;
pub mod router;
pub mod runtime;
pub mod schema;
#[cfg(feature = "storage")]
pub mod storage;
pub mod transport;
