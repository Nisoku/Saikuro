//! Runtime configuration.

use serde::{Deserialize, Serialize};
use std::time::Duration;

use saikuro_router::router::RouterConfig;
use saikuro_schema::registry::RegistryMode;
use saikuro_transport::selector::TransportConfig;

/// Top-level runtime configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Whether the runtime starts in development or production mode.
    #[serde(default)]
    pub mode: RuntimeMode,

    /// Maximum number of simultaneous in-flight calls across all providers.
    #[serde(default = "default_max_in_flight")]
    pub max_in_flight_calls: usize,

    /// Default timeout for `Call` invocations.
    #[serde(with = "duration_serde", default = "default_call_timeout")]
    pub call_timeout: Duration,

    /// Transport override (optional; if absent the selector auto-picks).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<TransportConfig>,

    /// Maximum wire message size in bytes. Defaults to 16 MiB.
    #[serde(default = "default_max_message_size")]
    pub max_message_size: usize,

    /// Buffer capacity for per-stream item queues.
    #[serde(default = "default_stream_capacity")]
    pub stream_buffer_capacity: usize,

    /// Enable structured JSON logging via `tracing-subscriber`.
    #[serde(default)]
    pub json_logs: bool,
}

impl RuntimeConfig {
    pub fn router_config(&self) -> RouterConfig {
        RouterConfig {
            call_timeout: self.call_timeout,
            stream_channel_capacity: self.stream_buffer_capacity,
            channel_capacity: self.stream_buffer_capacity,
        }
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            mode: RuntimeMode::Development,
            max_in_flight_calls: 1024,
            call_timeout: default_call_timeout(),
            transport: None,
            max_message_size: default_max_message_size(),
            stream_buffer_capacity: default_stream_capacity(),
            json_logs: false,
        }
    }
}

/// Whether the runtime runs in development or production mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeMode {
    /// Dynamic schema updates allowed; verbose logging; discovery enabled.
    #[default]
    Development,
    /// Schema frozen; production logging; discovery disabled.
    Production,
}

impl From<RuntimeMode> for RegistryMode {
    fn from(m: RuntimeMode) -> Self {
        match m {
            RuntimeMode::Development => RegistryMode::Development,
            RuntimeMode::Production => RegistryMode::Production,
        }
    }
}

fn default_max_in_flight() -> usize {
    1024
}
fn default_call_timeout() -> Duration {
    Duration::from_secs(30)
}
fn default_max_message_size() -> usize {
    16 * 1024 * 1024
}
fn default_stream_capacity() -> usize {
    128
}

/// Serde helper to (de)serialise `Duration` as milliseconds.
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(d.as_millis() as u64)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let ms = u64::deserialize(d)?;
        Ok(Duration::from_millis(ms))
    }
}
