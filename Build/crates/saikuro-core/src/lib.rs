//! Saikuro Core
//!
//! Foundational protocol types, envelope structures, and error definitions
//! for the Saikuro cross-language invocation fabric. Every other crate
//! in the workspace depends on this one; it purposely has minimal dependencies
//! and zero async code so it can be embedded anywhere.

pub mod capability;
pub mod envelope;
pub mod error;
pub mod invocation;
pub mod log;
pub mod resource;
pub mod schema;
pub mod value;

pub use capability::{CapabilitySet, CapabilityToken};
pub use envelope::{split_target, Envelope, InvocationType, ResponseEnvelope};
pub use error::{ErrorCode, ErrorDetail, SaikuroError};
pub use invocation::InvocationId;
pub use log::{stderr_log_sink, LogLevel, LogRecord, LogSink};
pub use resource::ResourceHandle;
pub use value::Value;

/// Wire-level protocol version. All envelopes carry this; the runtime
/// rejects messages with an incompatible version.
pub const PROTOCOL_VERSION: u32 = 1;
