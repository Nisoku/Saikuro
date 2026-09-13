use async_trait::async_trait;
use serde_json;

use crate::record::LogRecord;
use crate::sink::LogSink;

/// A sink emitting [`LogRecord`]s as JSON lines on stderr.
pub struct StderrSink;

#[async_trait]
impl LogSink for StderrSink {
    async fn emit(&self, record: &LogRecord) {
        if let Ok(json) = serde_json::to_string(record) {
            std::eprintln!("{}", json);
        }
    }
}

/// Construct a [`StderrSink`].
pub fn stderr_log_sink() -> StderrSink {
    StderrSink
}
