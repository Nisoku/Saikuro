//! Runtime error type.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("schema error: {0}")]
    Schema(String),

    #[error("transport error: {0}")]
    Transport(String),

    #[error("router error: {0}")]
    Router(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("capability denied: {0}")]
    CapabilityDenied(String),

    #[error("runtime already shut down")]
    Shutdown,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialisation error: {0}")]
    Serialisation(String),

    #[error("internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, RuntimeError>;

impl From<saikuro_schema::registry::RegistryError> for RuntimeError {
    fn from(e: saikuro_schema::registry::RegistryError) -> Self {
        Self::Schema(e.to_string())
    }
}

impl From<saikuro_transport::error::TransportError> for RuntimeError {
    fn from(e: saikuro_transport::error::TransportError) -> Self {
        Self::Transport(e.to_string())
    }
}

impl From<saikuro_router::error::RouterError> for RuntimeError {
    fn from(e: saikuro_router::error::RouterError) -> Self {
        Self::Router(e.to_string())
    }
}
