use alloc::string::String;
use core::fmt;
use core::str::FromStr;
use serde::{Deserialize, Serialize};

use saikuro_core::error::SaikuroError;
use saikuro_core::value::{Value, ValueMap};

use crate::level::LogLevel;

/// Maximum number of structured context fields a [`LogRecord`] can carry.
pub const LOG_FIELDS_CAPACITY: usize = 16;

/// Fixed-capacity map of structured context fields on [`LogRecord`].
pub type LogFieldMap = heapless::FnvIndexMap<String, Value, LOG_FIELDS_CAPACITY>;

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
    #[serde(default, skip_serializing_if = "LogFieldMap::is_empty")]
    pub fields: LogFieldMap,
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
            fields: LogFieldMap::new(),
        }
    }

    /// Add a structured field and return `self` for chaining.
    ///
    /// Fails with [`SaikuroError::CapacityExceeded`] if the record is already at
    /// [`LOG_FIELDS_CAPACITY`] fields.
    pub fn with_field(
        mut self,
        key: impl Into<String>,
        value: impl Into<Value>,
    ) -> Result<Self, SaikuroError> {
        let key = key.into();
        self.fields.insert(key.clone(), value.into()).map_err(|_| {
            SaikuroError::CapacityExceeded(format!("log field bag full at key '{key}'"))
        })?;
        Ok(self)
    }
}

/// Helper: extract a `Value::String` from a [`ValueMap`] by key.
fn take_string(map: &mut ValueMap, key: &str) -> Option<String> {
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
                        Value::String(s) => LogLevel::from_str(s.as_str()).ok(),
                        _ => None,
                    })
                    .unwrap_or(LogLevel::Info);
                let name = take_string(&mut map, "name").unwrap_or_default();
                let msg = take_string(&mut map, "msg").unwrap_or_default();
                let mut fields = LogFieldMap::new();
                for (k, v) in map.into_iter() {
                    fields
                        .insert(k, v)
                        .map_err(|_| "log record has too many fields")?;
                }
                Ok(LogRecord {
                    ts,
                    level,
                    name,
                    msg,
                    fields,
                })
            }
            _ => Err("expected a Map"),
        }
    }
}

impl fmt::Display for LogRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} {} :  {}", self.ts, self.level, self.name, self.msg)
    }
}
