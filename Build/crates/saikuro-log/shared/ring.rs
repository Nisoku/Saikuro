//! A bounded in-memory collector sink.

use alloc::vec::Vec;
use spin::Mutex;

use crate::record::LogRecord;
use crate::sink::LogSink;

/// A sink that retains the most recent records in a ring buffer, for later
/// inspection (e.g. a host draining buffered logs from a constrained target).
///
/// Bounded by `capacity`; once full, the oldest record is evicted.
pub struct RingSink {
    buffer: Mutex<Vec<LogRecord>>,
    capacity: usize,
}

impl RingSink {
    /// Create a collector that retains up to `capacity` records.
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Mutex::new(Vec::new()),
            capacity,
        }
    }

    /// Remove and return all buffered records.
    pub fn drain(&self) -> Vec<LogRecord> {
        self.buffer.lock().drain(..).collect()
    }
}

impl LogSink for RingSink {
    async fn emit(&self, record: &LogRecord) {
        let mut buf = self.buffer.lock();
        if buf.len() >= self.capacity {
            buf.remove(0);
        }
        buf.push(record.clone());
    }
}
