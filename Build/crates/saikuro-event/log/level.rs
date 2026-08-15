use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// Severity level of a log record, ordered from least to most severe.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    Display,
    EnumString,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum LogLevel {
    /// Most verbose: fine-grained debugging trace.
    Trace,
    /// Debugging information.
    Debug,
    /// Informational messages.
    Info,
    /// Warnings: recoverable anomalies.
    Warn,
    /// Errors: an operation failed.
    Error,
}
