//! Native (tokio-backed) I/O and networking primitives for Saikuro.

/// Async byte-stream I/O traits used by the native transport backends.
pub mod io;

/// Socket-address and network-stack types used by the native transport backends.
pub mod net;
