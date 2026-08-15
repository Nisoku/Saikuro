use core::fmt::Write as _;
use embedded_io_async::Write;
use heapless::String as HString;
use spin::Mutex;

use crate::record::LogRecord;
use crate::sink::LogSink;

/// A sink emitting [`LogRecord`]s as text lines on a serial writer.
///
/// `W` is any `embedded-io-async` writer (a UART/USART driver). Records are
/// formatted into a fixed-capacity stack buffer and written asynchronously.
pub struct SerialSink<W: Write> {
    writer: Mutex<W>,
}

impl<W: Write + Send> SerialSink<W> {
    /// Wrap a serial writer.
    pub fn new(writer: W) -> Self {
        Self {
            writer: Mutex::new(writer),
        }
    }
}

impl<W: Write + Send> LogSink for SerialSink<W> {
    async fn emit(&self, record: &LogRecord) {
        let mut buf = HString::<512>::new();
        if core::fmt::write(
            &mut buf,
            format_args!("[{}] {} {}", record.ts, record.level, record.msg),
        )
        .is_ok()
        {
            let bytes = buf.as_bytes();
            let mut offset = 0;
            while offset < bytes.len() {
                match self.writer.lock().write(&bytes[offset..]).await {
                    Ok(0) => break,
                    Ok(n) => offset += n,
                    Err(_) => break,
                }
            }
        }
    }
}

/// Construct a [`SerialSink`] wrapping `writer`.
pub fn serial_log_sink<W: Write + Send>(writer: W) -> SerialSink<W> {
    SerialSink::new(writer)
}
