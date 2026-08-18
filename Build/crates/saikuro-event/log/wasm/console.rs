#[cfg(feature = "console")]
use serde_json;
#[cfg(feature = "console")]
use wasm_bindgen::JsValue;

use crate::record::LogRecord;
use crate::sink::LogSink;

/// A sink emitting [`LogRecord`]s as JSON lines on the browser console.
pub struct ConsoleSink;

impl LogSink for ConsoleSink {
    async fn emit(&self, record: &LogRecord) {
        #[cfg(feature = "console")]
        {
            if let Ok(json) = serde_json::to_string(record) {
                web_sys::console::log_1(&JsValue::from_str(&json));
            }
        }
        #[cfg(not(feature = "console"))]
        {
            let _ = record;
        }
    }
}

/// Construct a [`ConsoleSink`].
pub fn console_log_sink() -> ConsoleSink {
    ConsoleSink
}
