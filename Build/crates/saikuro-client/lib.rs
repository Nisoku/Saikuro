//! Saikuro client library.
//!
//! Provides [`Provider`] and [`Client`]: the two main building blocks for
//! writing Rust services that connect to a Saikuro runtime.

#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

pub mod client;
pub mod provider;

pub use client::{Client, ClientOptions, SaikuroChannel, SaikuroStream};
pub use provider::{HandlerArgs, Provider, RegisterOptions};
pub use saikuro_core::schema::{PrimitiveType, TypeDescriptor, Visibility};
pub use saikuro_schema::builder::{ArgDescriptor, FunctionSchema, NamespaceSchema, build_schema};
pub use saikuro_transport::{AdapterTransport, MemoryAdapterTransport, connect};

/// The value type used throughout the Saikuro Rust adapter.
pub type Value = serde_json::Value;

pub use saikuro_event::{SaikuroError as Error, Result};
