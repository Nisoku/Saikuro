use alloc::{boxed::Box, string::String};
use core::fmt;
use core::str::FromStr;
use serde::{Deserialize, Serialize};

use crate::level::LogLevel;
use crate::value::{Value, ValueMap};
use crate::ContextMap;
use crate::SaikuroError;

/// A structured log record forwarded from an adapter to the runtime log sink.
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
    #[serde(default, skip_serializing_if = "ContextMap::is_empty")]
    pub fields: Box<ContextMap>,
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
            fields: Box::new(ContextMap::new()),
        }
    }

    /// Construct a log record with an auto-generated ISO-8601 timestamp.
    ///
    /// On `std` targets the current wall-clock time is used.  On `no_std` /
    /// `embedded` targets the timestamp is empty.
    #[cfg(feature = "std")]
    pub fn now(level: LogLevel, name: impl Into<String>, msg: impl Into<String>) -> Self {
        use std::time::SystemTime;
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| {
                let secs = d.as_secs();
                let millis = d.subsec_millis();
                format!("{secs:010}.{millis:03}")
            })
            .unwrap_or_default();
        Self::new(ts, level, name, msg)
    }

    /// Construct a log record with an auto-generated timestamp.
    ///
    /// On `no_std` / `embedded` targets the timestamp is empty.
    #[cfg(not(feature = "std"))]
    pub fn now(level: LogLevel, name: impl Into<String>, msg: impl Into<String>) -> Self {
        Self::new("", level, name, msg)
    }

    /// Add a structured field in-place, ignoring capacity errors.
    ///
    /// If the field bag is full the field is silently dropped.
    pub fn set_context(&mut self, key: impl Into<String>, value: impl Into<Value>) {
        let _ = self.fields.insert(key.into(), value.into());
    }

    /// Add a structured field and return `self` for chaining.
    ///
    /// Fails with [`SaikuroError::CapacityExceeded`] if the record is already at
    /// capacity fields.
    pub fn with_context(
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
                let mut fields = Box::new(ContextMap::new());
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
        write!(
            f,
            "[{}] {} {} :  {}",
            self.ts, self.level, self.name, self.msg
        )
    }
}
