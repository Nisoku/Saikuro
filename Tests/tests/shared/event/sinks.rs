use crate::shared_test;
#[allow(unused_imports)]
use crate::Box;
use crate::TestSuite;
use saikuro_core::Arc;
use saikuro_event::{IoError, IoErrorKind, LogLevel, LogRecord, LogSink, RingSink, Value};

/// A test-local sink that records every emitted message.
struct VecSink(Arc<spin::Mutex<crate::Vec<crate::String>>>);

impl VecSink {
    fn new() -> (Self, Arc<spin::Mutex<crate::Vec<crate::String>>>) {
        let messages = Arc::new(spin::Mutex::new(crate::Vec::new()));
        (Self(messages.clone()), messages)
    }
}

#[cfg_attr(not(feature = "embedded"), async_trait::async_trait)]
#[cfg_attr(feature = "embedded", async_trait::async_trait(?Send))]
impl LogSink for VecSink {
    async fn emit(&self, record: &LogRecord) {
        self.0
            .lock()
            .push(crate::format!("{:?}:{}", record.level, record.msg));
    }
}

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "event::level_filter_sink_blocks_below_threshold",
        level_filter_sink_blocks_below_threshold,
    );
    shared_test!(
        suite,
        "event::ring_sink_evicts_oldest_when_full",
        ring_sink_evicts_oldest_when_full,
    );
    shared_test!(
        suite,
        "event::ring_sink_drain_empty_is_noop",
        ring_sink_drain_empty_is_noop,
    );
    shared_test!(
        suite,
        "event::log_record_set_context_populates_fields",
        log_record_set_context_populates_fields,
    );
    shared_test!(
        suite,
        "event::log_record_with_context_chains_and_errors_on_full",
        log_record_with_context_chains_and_errors_on_full,
    );
    shared_test!(
        suite,
        "event::log_record_set_context_silently_drops_when_full",
        log_record_set_context_silently_drops_when_full,
    );
    shared_test!(
        suite,
        "event::log_record_display_frames",
        log_record_display_frames,
    );
    shared_test!(
        suite,
        "event::io_error_kind_display_strings",
        io_error_kind_display_strings,
    );
    shared_test!(
        suite,
        "event::io_error_display_with_and_without_message",
        io_error_display_with_and_without_message,
    );
}

fn record(name: &str, msg: &str, level: LogLevel) -> LogRecord {
    LogRecord::new("2026-01-01T00:00:00.000Z", level, name, msg)
}

fn level_filter_sink_blocks_below_threshold() -> Result<(), &'static str> {
    crate::block_on(async {
        let (inner, messages) = VecSink::new();
        let sink = saikuro_event::LevelFilterSink::new(inner, LogLevel::Warn);
        sink.emit(&record("t", "trace", LogLevel::Trace)).await;
        sink.emit(&record("i", "info", LogLevel::Info)).await;
        sink.emit(&record("e", "error", LogLevel::Error)).await;
        let captured = messages.lock().clone();
        crate::check_test!(
            captured.len() == 1,
            "only records at or above the threshold must be forwarded"
        );
        crate::check_test!(
            captured[0].ends_with("error"),
            "the forwarded record must be the error"
        );
        Ok(())
    })
}

fn ring_sink_evicts_oldest_when_full() -> Result<(), &'static str> {
    crate::block_on(async {
        let sink = RingSink::new(3);
        for i in 0..5 {
            sink.emit(&record("n", &crate::format!("msg{i}"), LogLevel::Info))
                .await;
        }
        let drained = sink.drain();
        crate::check_test!(
            drained.len() == 3,
            "ring sink must retain at most its capacity"
        );
        crate::check_test!(
            drained[0].msg == "msg2" && drained[2].msg == "msg4",
            "the oldest records must be evicted"
        );
        Ok(())
    })
}

fn ring_sink_drain_empty_is_noop() -> Result<(), &'static str> {
    crate::block_on(async {
        let sink = RingSink::new(4);
        let drained = sink.drain();
        crate::check_test!(drained.is_empty(), "a fresh sink must drain empty");
        Ok(())
    })
}

fn log_record_set_context_populates_fields() -> Result<(), &'static str> {
    let mut rec = record("n", "m", LogLevel::Info);
    rec.set_context("trace_id", Value::String("abc".into()));
    rec.set_context("attempt", Value::Int(3));
    let fields = rec.fields().ok_or("fields must be allocated")?;
    crate::check_test!(fields.len() == 2, "both context entries must be present");
    crate::check_test!(
        fields.get("trace_id") == Some(&Value::String("abc".into())),
        "string context must roundtrip"
    );
    crate::check_test!(
        fields.get("attempt") == Some(&Value::Int(3)),
        "int context must roundtrip"
    );
    Ok(())
}

fn log_record_with_context_chains_and_errors_on_full() -> Result<(), &'static str> {
    let mut rec = record("n", "m", LogLevel::Info);
    for i in 0..saikuro_event::CONTEXT_CAPACITY {
        rec = rec
            .with_context(crate::format!("k{i}"), Value::Int(i as i64))
            .map_err(|_| "with_context must accept capacity many entries")?;
    }
    let overflow = rec.with_context("overflow", Value::Null);
    crate::check_test!(
        matches!(
            overflow,
            Err(saikuro_event::SaikuroError::CapacityExceeded(_))
        ),
        "the entry past the field-bag capacity must error"
    );
    Ok(())
}

fn log_record_set_context_silently_drops_when_full() -> Result<(), &'static str> {
    let mut rec = record("n", "m", LogLevel::Info);
    for i in 0..saikuro_event::CONTEXT_CAPACITY {
        rec.set_context(crate::format!("k{i}"), Value::Int(i as i64));
    }
    rec.set_context("overflow", Value::Null);
    let fields = rec.fields().ok_or("fields must be allocated")?;
    crate::check_test!(
        fields.len() == saikuro_event::CONTEXT_CAPACITY,
        "set_context must not grow past capacity"
    );
    crate::check_test!(
        !fields.contains_key("overflow"),
        "the overflowing field must be dropped"
    );
    Ok(())
}

fn log_record_display_frames() -> Result<(), &'static str> {
    let rec = record("svc.handler", "boom", LogLevel::Error);
    let out = crate::format!("{rec}");
    crate::check_test!(
        out.contains("svc.handler") && out.contains("boom"),
        "display must include name and message"
    );
    crate::check_test!(out.contains("error"), "display must include the level");
    Ok(())
}

fn io_error_kind_display_strings() -> Result<(), &'static str> {
    let cases = [
        (IoErrorKind::NotFound, "not found"),
        (IoErrorKind::PermissionDenied, "permission denied"),
        (IoErrorKind::AlreadyExists, "entity already exists"),
        (IoErrorKind::ConnectionRefused, "connection refused"),
        (IoErrorKind::ConnectionReset, "connection reset"),
        (IoErrorKind::ConnectionAborted, "connection aborted"),
        (IoErrorKind::NotConnected, "not connected"),
        (IoErrorKind::AddrInUse, "address in use"),
        (IoErrorKind::AddrNotAvailable, "address not available"),
        (IoErrorKind::BrokenPipe, "broken pipe"),
        (IoErrorKind::WouldBlock, "operation would block"),
        (IoErrorKind::InvalidInput, "invalid input"),
        (IoErrorKind::InvalidData, "invalid data"),
        (IoErrorKind::TimedOut, "timed out"),
        (IoErrorKind::WriteZero, "write zero"),
        (IoErrorKind::Interrupted, "operation interrupted"),
        (IoErrorKind::UnexpectedEof, "unexpected end of file"),
        (IoErrorKind::OutOfMemory, "out of memory"),
        (IoErrorKind::Unsupported, "unsupported"),
        (IoErrorKind::Other, "other I/O error"),
    ];
    for (kind, expected) in cases {
        crate::check_test!(
            crate::format!("{kind}") == expected,
            "IoErrorKind display must be stable"
        );
    }
    Ok(())
}

fn io_error_display_with_and_without_message() -> Result<(), &'static str> {
    let bare = IoError {
        kind: IoErrorKind::TimedOut,
        message: None,
    };
    crate::check_test!(
        crate::format!("{bare}") == "timed out",
        "a message-less IoError must display its kind"
    );
    let with_msg = IoError {
        kind: IoErrorKind::TimedOut,
        message: Some("read took too long".into()),
    };
    crate::check_test!(
        crate::format!("{with_msg}") == "timed out: read took too long",
        "an IoError with a message must append it after a colon"
    );
    Ok(())
}
