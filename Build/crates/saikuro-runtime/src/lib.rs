#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[macro_use]
extern crate alloc;

pub mod config;
pub mod connection;
pub mod handle;
pub mod runtime;
pub mod transport_adapter;

pub use config::RuntimeConfig;
pub use handle::RuntimeHandle;
pub use runtime::SaikuroRuntime;
