//! Log-envelope dispatch tests

use futures::{pin_mut, poll};
use saikuro_core::{
    envelope::{Envelope, InvocationType},
    InvocationId, PROTOCOL_VERSION,
};
use saikuro_event::{LogLevel, LogRecord, LogSink, Value};
use saikuro_exec::mpsc;
use saikuro_router::{
    provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem},
    router::{InvocationRouter, RouterConfig},
};
use std::sync::{Arc, Mutex};
use std::task::Poll;

//  Helpers

struct CapturingSink {
    captured: Arc<Mutex<Vec<LogRecord>>>,
}

#[async_trait::async_trait]
impl LogSink for CapturingSink {
    async fn emit(&self, record: &LogRecord) {
        self.captured.lock().unwrap().push(record.clone());
    }
}

/// Build a capturing log sink that records every [`LogRecord`] it receives.
fn capturing_sink() -> (CapturingSink, Arc<Mutex<Vec<LogRecord>>>) {
    let captured: Arc<Mutex<Vec<LogRecord>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = CapturingSink {
        captured: captured.clone(),
    };
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
        id: InvocationId::new().expect("entropy available"),
        target: "$log".to_owned(),
        args: vec![value],
        meta: Default::default(),
        capability: None,
        batch_items: None,
        stream_control: None,
        seq: None,
    }
}

fn make_router_with_sink(sink: CapturingSink) -> InvocationRouter<CapturingSink> {
    let registry = ProviderRegistry::new(); // no providers needed for log tests
    InvocationRouter::<CapturingSink>::with_log_sink(registry, RouterConfig::default(), sink)
}

//  Tests

#[test]
fn log_envelope_is_not_routed_to_provider() {
    saikuro_exec::block_on(async {
        // Even with a registered provider, a Log envelope must NOT reach it.
        let (work_tx, mut work_rx) = mpsc::channel::<ProviderWorkItem>(
            saikuro_exec::ChannelCapacity::try_from(8).expect("8 is a valid channel capacity"),
        );
        let handle = ProviderHandle::new("logger", vec!["$log".to_owned()], work_tx);
        let registry = ProviderRegistry::new();
        registry.register(handle).await;

        let (sink, _captured) = capturing_sink();
        let router = InvocationRouter::<CapturingSink>::with_log_sink(
            registry,
            RouterConfig::default(),
            sink,
        );

        let env = make_log_envelope(LogLevel::Info, "test.logger", "hello from test");
        let resp = router.dispatch(env).await;

        // Must succeed.
        assert!(resp.ok, "log dispatch should return ok_empty");

        // Provider channel must be empty:  log was NOT forwarded to it.
        let recv_fut = work_rx.recv();
        pin_mut!(recv_fut);
        assert!(
            matches!(poll!(recv_fut.as_mut()), Poll::Pending),
            "log envelope must not be forwarded to any provider"
        );
    })
}

#[test]
fn log_envelope_delivers_record_to_sink() {
    saikuro_exec::block_on(async {
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
    })
}

#[test]
fn log_all_levels_are_forwarded() {
    saikuro_exec::block_on(async {
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
            assert!(resp.ok, "log dispatch for {level:?} should succeed");
        }

        let records = captured.lock().unwrap();
        assert_eq!(records.len(), levels.len(), "all levels should be captured");
        for (i, &expected_level) in levels.iter().enumerate() {
            assert_eq!(records[i].level, expected_level);
        }
    })
}

#[test]
fn log_envelope_with_no_args_returns_ok_without_panicking() {
    saikuro_exec::block_on(async {
        let (sink, captured) = capturing_sink();
        let router = make_router_with_sink(sink);

        // Malformed: no args at all.
        let env = Envelope {
            version: PROTOCOL_VERSION,
            invocation_type: InvocationType::Log,
            id: InvocationId::new().expect("entropy available"),
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

        // The router logs a warning about the malformed record.
        assert!(
            !captured.lock().unwrap().is_empty(),
            "malformed log should emit a warning to sink"
        );
    })
}

#[test]
fn log_envelope_with_invalid_args_returns_ok_without_panicking() {
    saikuro_exec::block_on(async {
        let (sink, captured) = capturing_sink();
        let router = make_router_with_sink(sink);

        // args[0] is a plain string:  not a LogRecord map.
        let env = Envelope {
            version: PROTOCOL_VERSION,
            invocation_type: InvocationType::Log,
            id: InvocationId::new().expect("entropy available"),
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
            !captured.lock().unwrap().is_empty(),
            "invalid log args should emit a warning to sink"
        );
    })
}

#[test]
fn router_with_custom_sink_still_routes_calls() {
    saikuro_exec::block_on(async {
        // A custom log sink must not interfere with normal call routing.
        let (work_tx, work_rx) = mpsc::channel::<ProviderWorkItem>(
            saikuro_exec::ChannelCapacity::try_from(8).expect("8 is a valid channel capacity"),
        );
        let handle = ProviderHandle::new("math", vec!["math".to_owned()], work_tx);
        let registry = ProviderRegistry::new();
        registry.register(handle).await;

        // Spawn an auto-responder.
        saikuro_exec::spawn(async move {
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
        let router = InvocationRouter::<CapturingSink>::with_log_sink(
            registry,
            RouterConfig::default(),
            sink,
        );

        let env = Envelope::call("math.compute", vec![]).expect("entropy available");
        let resp = router.dispatch(env).await;
        assert!(resp.ok, "call should still succeed with custom log sink");
        assert_eq!(resp.result, Some(Value::Int(99)));
    })
}

#[test]
fn multiple_log_envelopes_all_delivered_to_sink() {
    saikuro_exec::block_on(async {
        let (sink, captured) = capturing_sink();
        let router = make_router_with_sink(sink);

        for i in 0..10u32 {
            let env = make_log_envelope(LogLevel::Info, "bulk.test", &format!("message {i}"));
            let resp = router.dispatch(env).await;
            assert!(resp.ok);
        }

        let records = captured.lock().unwrap();
        assert_eq!(records.len(), 10, "all 10 log records should be captured");
        for (i, record) in records.iter().enumerate() {
            assert_eq!(record.msg, format!("message {i}"));
        }
    })
}
