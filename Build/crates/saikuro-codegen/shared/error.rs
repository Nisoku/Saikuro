use alloc::string::String;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("unsupported type for target language: {0}")]
    UnsupportedType(String),

    #[error("schema error: {0}")]
    Schema(String),

    #[error("template error: {0}")]
    Template(String),
}

pub type Result<T> = core::result::Result<T, CodegenError>;
