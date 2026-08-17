/// Logs to the process stderr stream.
pub mod stderr;

#[cfg(feature = "tracing")]
/// Bridges Saikuro logging into the `tracing` ecosystem.
pub mod tracing;
