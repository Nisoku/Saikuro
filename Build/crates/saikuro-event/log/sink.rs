#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::boxed::Box;

use async_trait::async_trait;

use crate::level::LogLevel;
use crate::record::LogRecord;

/// A destination for [`LogRecord`]s.
#[cfg(feature = "embedded")]
#[async_trait(?Send)]
pub trait LogSink: Send + Sync {
    /// Emit a single log record.
    async fn emit(&self, record: &LogRecord);
}

/// A destination for [`LogRecord`]s.
#[cfg(not(feature = "embedded"))]
#[async_trait]
pub trait LogSink: Send + Sync {
    /// Emit a single log record.
    async fn emit(&self, record: &LogRecord);
}

/// A sink that discards every record.
///
/// Useful for benchmarks, silent embedded builds, and tests.
pub struct NullSink;

#[cfg(feature = "embedded")]
#[async_trait(?Send)]
impl LogSink for NullSink {
    async fn emit(&self, _record: &LogRecord) {}
}

#[cfg(not(feature = "embedded"))]
#[async_trait]
impl LogSink for NullSink {
    async fn emit(&self, _record: &LogRecord) {}
}

/// Wraps another sink, forwarding only records at or above `min_level`.
///
/// This is the simplest example of a *composable* sink: sinks can be layered
/// without knowing each other's internals.
pub struct LevelFilterSink<S> {
    inner: S,
    min_level: LogLevel,
}

impl<S> LevelFilterSink<S> {
    /// Wrap `inner`, dropping records below `min_level`.
    pub fn new(inner: S, min_level: LogLevel) -> Self {
        Self { inner, min_level }
    }
}

#[cfg(feature = "embedded")]
#[async_trait(?Send)]
impl<S: LogSink> LogSink for LevelFilterSink<S> {
    async fn emit(&self, record: &LogRecord) {
        if record.level >= self.min_level {
            self.inner.emit(record).await;
        }
    }
}

#[cfg(not(feature = "embedded"))]
#[async_trait]
impl<S: LogSink> LogSink for LevelFilterSink<S> {
    async fn emit(&self, record: &LogRecord) {
        if record.level >= self.min_level {
            self.inner.emit(record).await;
        }
    }
}
