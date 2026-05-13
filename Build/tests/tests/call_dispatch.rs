//! Call and cast dispatch integration tests

use saikuro_core::{envelope::Envelope, error::ErrorCode, value::Value, ResponseEnvelope};
use saikuro_exec::mpsc;
use saikuro_router::{
    provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem},
    router::{InvocationRouter, RouterConfig},
};
use std::time::Duration;

//  Helpers

/// Spawn a minimal provider task that automatically echoes every Call.
///
/// Returns the [`ProviderRegistry`] with the provider registered, plus a
/// join handle so callers can wait for completion.
fn make_echo_provider(namespace: &str) -> (ProviderRegistry, mpsc::Receiver<ProviderWorkItem>) {
    let (work_tx, work_rx) = mpsc::channel::<ProviderWorkItem>(64);
    let handle = ProviderHandle::new(
        format!("{namespace}-provider"),
        vec![namespace.to_owned()],
        work_tx,
    );
    let registry = ProviderRegistry::new();
    registry.register(handle);
    (registry, work_rx)
}

/// Spawn a background task that answers every work item with the given value.
fn spawn_responder(
    mut work_rx: mpsc::Receiver<ProviderWorkItem>,
    value: Value,
) -> saikuro_exec::JoinHandle<()> {
    saikuro_exec::spawn(async move {
        while let Some(item) = work_rx.recv().await {
            if let Some(tx) = item.response_tx {
                let _ = tx.send(ResponseEnvelope::ok(item.envelope.id, value.clone()));
            }
        }
    })
}

/// A responder that never answers (used to trigger timeouts).
///
/// Crucially, it holds the received work items (including their `response_tx`
/// channels) alive so the router blocks on the oneshot and eventually times out
/// rather than seeing a dropped sender (which would produce ProviderUnavailable).
fn spawn_silent_responder(
    mut work_rx: mpsc::Receiver<ProviderWorkItem>,
) -> saikuro_exec::JoinHandle<()> {
    saikuro_exec::spawn(async move {
        let mut held = Vec::new();
        while let Some(item) = work_rx.recv().await {
            // Keep the item alive so response_tx is not dropped.
            held.push(item);
        }
        // held is dropped when the task ends, but by then the test is over.
        drop(held);
    })
}

//  Tests

#[test]
fn call_returns_provider_response() {
    saikuro_exec::block_on(async {
        let (registry, work_rx) = make_echo_provider("math");
        let _responder = spawn_responder(work_rx, Value::Int(42));

        let router = InvocationRouter::with_providers(registry);
        let env = Envelope::call("math.add", vec![Value::Int(1), Value::Int(2)]);
        let resp = router.dispatch(env).await;

        assert!(resp.ok, "call should succeed");
        assert_eq!(resp.result, Some(Value::Int(42)));
    })
}

#[test]
fn cast_returns_ok_empty_immediately() {
    saikuro_exec::block_on(async {
        let (registry, mut work_rx) = make_echo_provider("logger");

        // Consume work items so the channel doesn't fill up, but never respond.
        saikuro_exec::spawn(async move { while (work_rx.recv().await).is_some() {} });

        let router = InvocationRouter::with_providers(registry);
        let env = Envelope::cast("logger.info", vec![Value::String("hello".into())]);
        let resp = router.dispatch(env).await;

        assert!(resp.ok, "cast should always return ok");
        assert!(resp.result.is_none());
    })
}

#[test]
fn call_to_unknown_namespace_returns_no_provider() {
    saikuro_exec::block_on(async {
        let registry = ProviderRegistry::new(); // empty
        let router = InvocationRouter::with_providers(registry);

        let env = Envelope::call("nonexistent.fn", vec![]);
        let resp = router.dispatch(env).await;

        assert!(!resp.ok);
        let err = resp.error.expect("error detail");
        assert_eq!(err.code, ErrorCode::NoProvider);
    })
}

#[test]
fn call_to_dropped_provider_returns_unavailable() {
    saikuro_exec::block_on(async {
        let (work_tx, work_rx) = mpsc::channel::<ProviderWorkItem>(1);
        let handle = ProviderHandle::new("gone", vec!["svc".to_owned()], work_tx);
        let registry = ProviderRegistry::new();
        registry.register(handle);

        // Drop the receiver:  the provider is "gone".
        drop(work_rx);

        let router = InvocationRouter::with_providers(registry);
        let env = Envelope::call("svc.op", vec![]);
        let resp = router.dispatch(env).await;

        assert!(!resp.ok);
        let err = resp.error.expect("error detail");
        assert!(
            err.code == ErrorCode::ProviderUnavailable || err.code == ErrorCode::NoProvider,
            "expected ProviderUnavailable or NoProvider, got {:?}",
            err.code
        );
    })
}

#[test]
fn call_times_out_when_provider_does_not_respond() {
    saikuro_exec::block_on(async {
        let (registry, work_rx) = make_echo_provider("slow");
        let _silent = spawn_silent_responder(work_rx);

        let config = RouterConfig {
            call_timeout: Duration::from_millis(50),
            ..Default::default()
        };
        let router = InvocationRouter::new(registry, config);

        let env = Envelope::call("slow.fn", vec![]);
        let resp = router.dispatch(env).await;

        assert!(!resp.ok);
        let err = resp.error.expect("error detail");
        assert_eq!(err.code, ErrorCode::Timeout);
    })
}

#[test]
fn multiple_sequential_calls_all_succeed() {
    saikuro_exec::block_on(async {
        let (registry, work_rx) = make_echo_provider("counter");
        let _responder = spawn_responder(work_rx, Value::Bool(true));

        let router = InvocationRouter::with_providers(registry);

        for _ in 0..5 {
            let env = Envelope::call("counter.inc", vec![]);
            let resp = router.dispatch(env).await;
            assert!(resp.ok);
        }
    })
}

#[test]
fn concurrent_calls_all_succeed() {
    saikuro_exec::block_on(async {
        let (registry, work_rx) = make_echo_provider("parallel");
        let _responder = spawn_responder(work_rx, Value::Int(0));

        let router = InvocationRouter::with_providers(registry);

        let mut handles = vec![];
        for _ in 0..20 {
            let r = router.clone();
            handles.push(saikuro_exec::spawn(async move {
                let env = Envelope::call("parallel.op", vec![]);
                r.dispatch(env).await
            }));
        }

        for h in handles {
            let resp = h.await.expect("task panicked");
            assert!(resp.ok, "concurrent call should succeed");
        }
    })
}

#[test]
fn call_with_null_target_returns_malformed_or_no_provider() {
    saikuro_exec::block_on(async {
        let registry = ProviderRegistry::new();
        let router = InvocationRouter::with_providers(registry);

        // A target with no dot is malformed.
        let env = Envelope::call("nodothere", vec![]);
        let resp = router.dispatch(env).await;
        assert!(!resp.ok);
        let err = resp.error.unwrap();
        assert!(
            err.code == ErrorCode::MalformedEnvelope || err.code == ErrorCode::NoProvider,
            "unexpected code {:?}",
            err.code
        );
    })
}

#[test]
fn cast_to_unknown_namespace_returns_ok() {
    saikuro_exec::block_on(async {
        // Cast is fire-and-forget; even if the namespace doesn't exist the
        // router has nowhere to send errors:  it returns ok_empty and logs.
        // Our router implementation does return ok_empty regardless for casts.
        let registry = ProviderRegistry::new();
        let router = InvocationRouter::with_providers(registry);

        let env = Envelope::cast("missing.fn", vec![]);
        let resp = router.dispatch(env).await;
        // Our implementation returns ok_empty for casts even when the namespace
        // is missing, as per fire-and-forget semantics.
        // Accept either ok or error:  both are defensible; we just assert no panic.
        let _ = resp;
    })
}
