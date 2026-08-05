//! Transport error type.
//!
//! The crate is `no_std` + `alloc` without the `std` feature, so the raw
//! `std::io::Error` variant is gated the same way as in saikuro-core.

use alloc::string::String;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("connection refused: {0}")]
    ConnectionRefused(String),

    #[error("connection lost: {0}")]
    ConnectionLost(String),

    #[error("send failed: {0}")]
    SendFailed(String),

    #[error("receive failed: {0}")]
    ReceiveFailed(String),

    #[error("message too large: {size} bytes, limit {limit}")]
    MessageTooLarge { size: usize, limit: usize },

    #[error("framing error: {0}")]
    FramingError(String),

    #[error("transport not supported on this platform")]
    NotSupported,

    #[cfg(feature = "std")]
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("msgpack encode error: {0}")]
    MsgpackEncode(#[from] saikuro_core::msgpack::EncodeError),

    #[error("msgpack decode error: {0}")]
    MsgpackDecode(#[from] saikuro_core::msgpack::DecodeError),

    #[error("channel closed")]
    ChannelClosed,
}

pub type Result<T> = core::result::Result<T, TransportError>;
