use alloc::string::String;
#[cfg(feature = "std")]
use alloc::string::ToString;
use core::fmt;

use serde::{Deserialize, Serialize};

/// Classification of an I/O failure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IoErrorKind {
    /// An entity was not found.
    NotFound,
    /// The operation lacked the necessary permissions.
    PermissionDenied,
    /// An entity already exists.
    AlreadyExists,
    /// The connection was refused.
    ConnectionRefused,
    /// The connection was reset by the remote side.
    ConnectionReset,
    /// The connection was aborted by the remote side.
    ConnectionAborted,
    /// The endpoint was not connected.
    NotConnected,
    /// A network address was already in use.
    AddrInUse,
    /// A network address was not available.
    AddrNotAvailable,
    /// The operating-system pipe was closed.
    BrokenPipe,
    /// The operation would block, but the caller asked for non-blocking.
    WouldBlock,
    /// Invalid input argument.
    InvalidInput,
    /// Invalid data encountered.
    InvalidData,
    /// The operation timed out.
    TimedOut,
    /// A write to a closed pipe or socket returned zero bytes.
    WriteZero,
    /// The operation was interrupted before completion.
    Interrupted,
    /// An unexpected end of file was encountered.
    UnexpectedEof,
    /// A memory allocation failed.
    OutOfMemory,
    /// The operation is not supported on this target.
    Unsupported,
    /// A category not covered by the variants above.
    Other,
}

impl fmt::Display for IoErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            IoErrorKind::NotFound => "not found",
            IoErrorKind::PermissionDenied => "permission denied",
            IoErrorKind::AlreadyExists => "entity already exists",
            IoErrorKind::ConnectionRefused => "connection refused",
            IoErrorKind::ConnectionReset => "connection reset",
            IoErrorKind::ConnectionAborted => "connection aborted",
            IoErrorKind::NotConnected => "not connected",
            IoErrorKind::AddrInUse => "address in use",
            IoErrorKind::AddrNotAvailable => "address not available",
            IoErrorKind::BrokenPipe => "broken pipe",
            IoErrorKind::WouldBlock => "operation would block",
            IoErrorKind::InvalidInput => "invalid input",
            IoErrorKind::InvalidData => "invalid data",
            IoErrorKind::TimedOut => "timed out",
            IoErrorKind::WriteZero => "write zero",
            IoErrorKind::Interrupted => "operation interrupted",
            IoErrorKind::UnexpectedEof => "unexpected end of file",
            IoErrorKind::OutOfMemory => "out of memory",
            IoErrorKind::Unsupported => "unsupported",
            IoErrorKind::Other => "other I/O error",
        };
        f.write_str(name)
    }
}

/// A portable I/O error: an [`IoErrorKind`] plus an optional human message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IoError {
    /// The failure category.
    pub kind: IoErrorKind,
    /// Optional free-form description, when the platform can supply one.
    pub message: Option<String>,
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.message {
            Some(msg) => write!(f, "{}: {}", self.kind, msg),
            None => write!(f, "{}", self.kind),
        }
    }
}

#[cfg(feature = "std")]
impl From<std::io::ErrorKind> for IoErrorKind {
    fn from(kind: std::io::ErrorKind) -> Self {
        match kind {
            std::io::ErrorKind::NotFound => IoErrorKind::NotFound,
            std::io::ErrorKind::PermissionDenied => IoErrorKind::PermissionDenied,
            std::io::ErrorKind::AlreadyExists => IoErrorKind::AlreadyExists,
            std::io::ErrorKind::ConnectionRefused => IoErrorKind::ConnectionRefused,
            std::io::ErrorKind::ConnectionReset => IoErrorKind::ConnectionReset,
            std::io::ErrorKind::ConnectionAborted => IoErrorKind::ConnectionAborted,
            std::io::ErrorKind::NotConnected => IoErrorKind::NotConnected,
            std::io::ErrorKind::AddrInUse => IoErrorKind::AddrInUse,
            std::io::ErrorKind::AddrNotAvailable => IoErrorKind::AddrNotAvailable,
            std::io::ErrorKind::BrokenPipe => IoErrorKind::BrokenPipe,
            std::io::ErrorKind::WouldBlock => IoErrorKind::WouldBlock,
            std::io::ErrorKind::InvalidInput => IoErrorKind::InvalidInput,
            std::io::ErrorKind::InvalidData => IoErrorKind::InvalidData,
            std::io::ErrorKind::TimedOut => IoErrorKind::TimedOut,
            std::io::ErrorKind::WriteZero => IoErrorKind::WriteZero,
            std::io::ErrorKind::Interrupted => IoErrorKind::Interrupted,
            std::io::ErrorKind::UnexpectedEof => IoErrorKind::UnexpectedEof,
            std::io::ErrorKind::OutOfMemory => IoErrorKind::OutOfMemory,
            std::io::ErrorKind::Unsupported => IoErrorKind::Unsupported,
            _ => IoErrorKind::Other,
        }
    }
}

#[cfg(feature = "std")]
impl From<std::io::Error> for IoError {
    fn from(err: std::io::Error) -> Self {
        IoError {
            kind: err.kind().into(),
            message: Some(err.to_string()),
        }
    }
}
