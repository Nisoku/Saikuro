#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

//! Unified error taxonomy, structured logging, and dynamically-typed value
//! types for Saikuro.
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod value;
pub use value::*;

mod core_events;
pub use core_events::*;

pub mod log;
pub use log::*;
