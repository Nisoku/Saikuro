//! Codegen error type.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("unsupported type for target language: {0}")]
    UnsupportedType(String),

    #[error("schema error: {0}")]
    Schema(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("template error: {0}")]
    Template(String),
}

pub type Result<T> = std::result::Result<T, CodegenError>;
