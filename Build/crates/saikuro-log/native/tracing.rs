use crate::level::LogLevel;
use crate::record::LogRecord;
use crate::sink::LogSink;

/// A sink emitting [`LogRecord`]s through the `tracing` crate at the matching
/// level.
pub struct TracingSink;

impl LogSink for TracingSink {
    async fn emit(&self, record: &LogRecord) {
        let line = format!("[{}] {}", record.name, record.msg);
        match record.level {
            LogLevel::Trace => tracing::trace!("{}", line),
            LogLevel::Debug => tracing::debug!("{}", line),
            LogLevel::Info => tracing::info!("{}", line),
            LogLevel::Warn => tracing::warn!("{}", line),
            LogLevel::Error => tracing::error!("{}", line),
        }
    }
}

/// Construct a [`TracingSink`].
pub fn tracing_log_sink() -> TracingSink {
    TracingSink
}
