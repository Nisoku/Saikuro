//! Log-envelope dispatch tests

use saikuro_core::{
    envelope::{Envelope, InvocationType},
    log::{LogLevel, LogRecord, LogSink},
    value::Value,
    InvocationId, PROTOCOL_VERSION,
};
use saikuro_router::{
    provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem},
    router::{InvocationRouter, RouterConfig},
};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

//  Helpers

/// Build a capturing log sink that records every [`LogRecord`] it receives.
fn capturing_sink() -> (LogSink, Arc<Mutex<Vec<LogRecord>>>) {
    let captured: Arc<Mutex<Vec<LogRecord>>> = Arc::new(Mutex::new(Vec::new()));
    let cap_clone = Arc::clone(&captured);
    let sink: LogSink = Box::new(move |record: LogRecord| {
        cap_clone.lock().unwrap().push(record);
    });
    (sink, captured)
}

/// Construct a `Log`-type envelope carrying a [`LogRecord`].
fn make_log_envelope(level: LogLevel, name: &str, msg: &str) -> Envelope {
    let record = LogRecord::new("2026-01-01T00:00:00.000Z", level, name, msg);
    // Serialise the record to a Value::Map so it can live in args.
    let bytes = rmp_serde::to_vec_named(&record).expect("serialize LogRecord");
    let value: Value = rmp_serde::from_slice(&bytes).expect("deserialize to Value");

    Envelope {
        version: PROTOCOL_VERSION,
        invocation_type: InvocationType::Log,
        id: InvocationId::new(),
        target: "$log".to_owned(),
        args: vec![value],
        meta: Default::default(),
        capability: None,
        batch_items: None,
        stream_control: None,
        seq: None,
    }
}

fn make_router_with_sink(sink: LogSink) -> InvocationRouter {
    let registry = ProviderRegistry::new(); // no providers needed for log tests
    InvocationRouter::with_log_sink(registry, RouterConfig::default(), sink)
}

//  Tests

#[tokio::test]
async fn log_envelope_is_not_routed_to_provider() {
    // Even with a registered provider, a Log envelope must NOT reach it.
    let (work_tx, mut work_rx) = mpsc::channel::<ProviderWorkItem>(8);
    let handle = ProviderHandle::new("logger", vec!["$log".to_owned()], work_tx);
    let registry = ProviderRegistry::new();
    registry.register(handle);

    let (sink, _captured) = capturing_sink();
    let router = InvocationRouter::with_log_sink(registry, RouterConfig::default(), sink);

    let env = make_log_envelope(LogLevel::Info, "test.logger", "hello from test");
    let resp = router.dispatch(env).await;

    // Must succeed.
    assert!(resp.ok, "log dispatch should return ok_empty");

    // Provider channel must be empty:  log was NOT forwarded to it.
    assert!(
        work_rx.try_recv().is_err(),
        "log envelope must not be forwarded to any provider"
    );
}

#[tokio::test]
async fn log_envelope_delivers_record_to_sink() {
    let (sink, captured) = capturing_sink();
    let router = make_router_with_sink(sink);

    let env = make_log_envelope(LogLevel::Warn, "myapp.handler", "something fishy");
    let resp = router.dispatch(env).await;

    assert!(resp.ok);

    let records = captured.lock().unwrap();
    assert_eq!(records.len(), 1, "exactly one record should be captured");
    let r = &records[0];
    assert_eq!(r.level, LogLevel::Warn);
    assert_eq!(r.name, "myapp.handler");
    assert_eq!(r.msg, "something fishy");
}

#[tokio::test]
async fn log_all_levels_are_forwarded() {
    let (sink, captured) = capturing_sink();
    let router = make_router_with_sink(sink);

    let levels = [
        LogLevel::Trace,
        LogLevel::Debug,
        LogLevel::Info,
        LogLevel::Warn,
        LogLevel::Error,
    ];

    for &level in &levels {
        let env = make_log_envelope(level, "level.test", "msg");
        let resp = router.dispatch(env).await;
        assert!(resp.ok, "log dispatch for {:?} should succeed", level);
    }

    let records = captured.lock().unwrap();
    assert_eq!(records.len(), levels.len(), "all levels should be captured");
    for (i, &expected_level) in levels.iter().enumerate() {
        assert_eq!(records[i].level, expected_level);
    }
}

#[tokio::test]
async fn log_envelope_with_no_args_returns_ok_without_panicking() {
    let (sink, captured) = capturing_sink();
    let router = make_router_with_sink(sink);

    // Malformed: no args at all.
    let env = Envelope {
        version: PROTOCOL_VERSION,
        invocation_type: InvocationType::Log,
        id: InvocationId::new(),
        target: "$log".to_owned(),
        args: vec![],
        meta: Default::default(),
        capability: None,
        batch_items: None,
        stream_control: None,
        seq: None,
    };
    let resp = router.dispatch(env).await;

    // Must not panic; ok_empty is returned.
    assert!(resp.ok, "malformed log should still return ok");

    // Nothing was delivered to the sink.
    assert!(
        captured.lock().unwrap().is_empty(),
        "malformed log should not reach sink"
    );
}

#[tokio::test]
async fn log_envelope_with_invalid_args_returns_ok_without_panicking() {
    let (sink, captured) = capturing_sink();
    let router = make_router_with_sink(sink);

    // args[0] is a plain string:  not a LogRecord map.
    let env = Envelope {
        version: PROTOCOL_VERSION,
        invocation_type: InvocationType::Log,
        id: InvocationId::new(),
        target: "$log".to_owned(),
        args: vec![Value::String("not a log record".into())],
        meta: Default::default(),
        capability: None,
        batch_items: None,
        stream_control: None,
        seq: None,
    };
    let resp = router.dispatch(env).await;

    assert!(resp.ok, "invalid log args should still return ok");
    assert!(
        captured.lock().unwrap().is_empty(),
        "invalid log args should not reach sink"
    );
}

#[tokio::test]
async fn router_with_custom_sink_still_routes_calls() {
    // A custom log sink must not interfere with normal call routing.
    let (work_tx, work_rx) = mpsc::channel::<ProviderWorkItem>(8);
    let handle = ProviderHandle::new("math", vec!["math".to_owned()], work_tx);
    let registry = ProviderRegistry::new();
    registry.register(handle);

    // Spawn an auto-responder.
    tokio::spawn(async move {
        let mut rx = work_rx;
        while let Some(item) = rx.recv().await {
            if let Some(tx) = item.response_tx {
                let _ = tx.send(saikuro_core::ResponseEnvelope::ok(
                    item.envelope.id,
                    Value::Int(99),
                ));
            }
        }
    });

    let (sink, _captured) = capturing_sink();
    let router = InvocationRouter::with_log_sink(registry, RouterConfig::default(), sink);

    let env = Envelope::call("math.compute", vec![]);
    let resp = router.dispatch(env).await;
    assert!(resp.ok, "call should still succeed with custom log sink");
    assert_eq!(resp.result, Some(Value::Int(99)));
}

#[tokio::test]
async fn multiple_log_envelopes_all_delivered_to_sink() {
    let (sink, captured) = capturing_sink();
    let router = make_router_with_sink(sink);

    for i in 0..10u32 {
        let env = make_log_envelope(LogLevel::Info, "bulk.test", &format!("message {}", i));
        let resp = router.dispatch(env).await;
        assert!(resp.ok);
    }

    let records = captured.lock().unwrap();
    assert_eq!(records.len(), 10, "all 10 log records should be captured");
    for (i, record) in records.iter().enumerate() {
        assert_eq!(record.msg, format!("message {}", i));
    }
}
