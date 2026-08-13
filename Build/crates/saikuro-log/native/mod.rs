//! Host (OS) logging sinks.

pub mod stderr;

#[cfg(feature = "tracing")]
pub mod tracing;
