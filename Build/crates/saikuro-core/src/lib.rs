//! Saikuro Core
//!
//! Foundational protocol types, envelope structures, and error definitions
//! for the Saikuro cross-language invocation fabric. Every other crate
//! in the workspace depends on this one; it purposely has minimal dependencies
//! and zero async code so it can be embedded anywhere.
//!
//! The crate is always `#![no_std]` + `alloc`: strings and vectors come from
//! `alloc`, and all maps/sets are fixed-capacity `heapless` collections. The
//! msgpack codec is available on every target; the default `std` feature adds
//! the `Io` error variant, a stderr log sink, and std-backed sync primitives.

#![no_std]

#[macro_use]
extern crate alloc;

#[cfg(any(feature = "std", feature = "std-no-os"))]
extern crate std;

pub mod capability;
pub mod envelope;
pub mod error;
pub mod invocation;
pub mod log;
pub mod msgpack;
pub mod resource;
pub mod schema;
pub mod sync;
pub mod value;

pub use capability::{CapabilitySet, CapabilityToken};
pub use envelope::{split_target, Envelope, InvocationType, ResponseEnvelope};
pub use error::{ErrorCode, ErrorDetail, SaikuroError};
pub use invocation::InvocationId;
#[cfg(any(feature = "std", feature = "std-no-os"))]
pub use log::stderr_log_sink;
pub use log::{LogLevel, LogRecord, LogSink};
pub use resource::ResourceHandle;
pub use value::Value;

/// Wire-level protocol version. All envelopes carry this; the runtime
/// rejects messages with an incompatible version.
pub const PROTOCOL_VERSION: u32 = 1;
