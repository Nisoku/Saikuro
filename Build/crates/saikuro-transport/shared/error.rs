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

    #[error("channel closed")]
    ChannelClosed,
}

pub type Result<T> = core::result::Result<T, TransportError>;

impl From<TransportError> for saikuro_event::SaikuroError {
    fn from(err: TransportError) -> Self {
        use saikuro_event::SaikuroError;
        match err {
            TransportError::ConnectionRefused(s) => SaikuroError::ConnectionRefused(s),
            TransportError::ConnectionLost(s) => SaikuroError::ConnectionLost(s),
            TransportError::SendFailed(s) => SaikuroError::SendFailed(s),
            TransportError::ReceiveFailed(s) => SaikuroError::ReceiveFailed(s),
            TransportError::MessageTooLarge { size, limit } => SaikuroError::MessageTooLarge { size, limit },
            TransportError::FramingError(s) => SaikuroError::FramingError(s),
            TransportError::NotSupported => SaikuroError::TransportNotSupported,
            TransportError::ChannelClosed => SaikuroError::ChannelClosed,
            #[cfg(feature = "std")]
            TransportError::Io(e) => SaikuroError::Io(e.into()),
        }
    }
}
