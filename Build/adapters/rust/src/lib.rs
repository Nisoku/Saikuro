//! Saikuro Rust Adapter
//!
//! This crate provides [`Provider`] and [`Client`]: the two main building
//! blocks for writing Rust services that connect to a Saikuro runtime.
//!
//! For testing without a live runtime use [`transport::InMemoryTransport`].

pub mod client;
pub mod error;
pub mod provider;
pub mod schema;
pub mod transport;
pub mod value;

#[cfg(any(feature = "storage", feature = "wasm-storage"))]
pub mod storage;

pub use client::{Client, ClientOptions, SaikuroChannel, SaikuroStream};
pub use error::{Error, Result};
pub use provider::{HandlerArgs, Provider, RegisterOptions};
pub use schema::{ArgDescriptor, FunctionSchema, NamespaceSchema};
pub use transport::InMemoryTransport;
pub use value::Value;

#[cfg(any(feature = "storage", feature = "wasm-storage"))]
pub use storage::{create_storage, create_transient_storage};
