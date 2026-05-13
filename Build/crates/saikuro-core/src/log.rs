//! Structured log record types for the Saikuro log-transport protocol.
//!
//! When an adapter wants to forward structured logs to the runtime (rather than
//! writing directly to its own stderr), it wraps a [`LogRecord`] in a standard
//! [`Envelope`](crate::envelope::Envelope) with
//! `invocation_type = InvocationType::Log` and places the serialised
//! `LogRecord` as the first element of `args`.
//!
//! The runtime's router intercepts `Log` envelopes before they reach a
//! provider and dispatches them to the configured [`LogSink`].

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::value::Value;

//  Log level

/// Severity level of a log record, ordered from least to most severe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl LogLevel {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

impl TryFrom<&str> for LogLevel {
    type Error = ();
    fn try_from(s: &str) -> Result<Self, <Self as TryFrom<&str>>::Error> {
        match s {
            "trace" => Ok(Self::Trace),
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" => Ok(Self::Warn),
            "error" => Ok(Self::Error),
            _ => Err(()),
        }
    }
}

//  Log record

/// A structured log record forwarded from an adapter to the runtime log sink.
///
/// The `fields` map holds any additional key/value context the emitting logger
/// attached (e.g. `err`, `id`, `duration_ms`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRecord {
    /// ISO-8601 timestamp string (e.g. `"2026-01-01T00:00:00.000Z"`).
    pub ts: String,

    /// Severity level.
    pub level: LogLevel,

    /// Logger name / origin (e.g. `"saikuro.transport"`, `"myapp.handler"`).
    pub name: String,

    /// Human-readable message.
    pub msg: String,

    /// Additional structured context fields.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Value>,
}

impl LogRecord {
    /// Construct a minimal log record with no extra fields.
    pub fn new(
        ts: impl Into<String>,
        level: LogLevel,
        name: impl Into<String>,
        msg: impl Into<String>,
    ) -> Self {
        Self {
            ts: ts.into(),
            level,
            name: name.into(),
            msg: msg.into(),
            fields: BTreeMap::new(),
        }
    }

    /// Add a structured field and return `self` for chaining.
    pub fn with_field(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.fields.insert(key.into(), value.into());
        self
    }
}

/// Helper: extract a `Value::String` from a map by key.
fn take_string(map: &mut BTreeMap<String, Value>, key: &str) -> Option<String> {
    match map.remove(key) {
        Some(Value::String(s)) => Some(s),
        _ => None,
    }
}

impl TryFrom<Value> for LogRecord {
    type Error = &'static str;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Map(mut map) => {
                let ts = take_string(&mut map, "ts").unwrap_or_default();
                let level = map
                    .remove("level")
                    .and_then(|v| match v {
                        Value::String(s) => LogLevel::try_from(s.as_str()).ok(),
                        _ => None,
                    })
                    .unwrap_or(LogLevel::Info);
                let name = take_string(&mut map, "name").unwrap_or_default();
                let msg = take_string(&mut map, "msg").unwrap_or_default();
                Ok(LogRecord {
                    ts,
                    level,
                    name,
                    msg,
                    fields: map,
                })
            }
            _ => Err("expected a Map"),
        }
    }
}

impl std::fmt::Display for LogRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} {} :  {}",
            self.ts, self.level, self.name, self.msg
        )
    }
}

//  Log sink

/// A callable that receives log records forwarded by adapters.
///
/// Construct a concrete sink with [`stderr_log_sink`] (writes JSON lines to
/// stderr) or build your own by implementing the same signature.
///
/// Higher-level crates (`saikuro-runtime`) provide a `tracing`-backed default.
pub type LogSink = Box<dyn Fn(LogRecord) + Send + Sync + 'static>;

/// A simple log sink that serialises each [`LogRecord`] as a JSON line and
/// writes it to stderr.  Used when no richer sink is configured.
pub fn stderr_log_sink() -> LogSink {
    Box::new(|record: LogRecord| {
        // Minimal JSON serialisation without pulling in serde_json :  just emit
        // the Display representation which is always human-readable.
        eprintln!("[saikuro] {record}");
    })
}
