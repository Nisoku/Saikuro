
use crate::common;
use crate::TestSuite;
use saikuro_core::envelope::Envelope;
use saikuro_event::{ErrorCode, Value};
use saikuro_exec::mpsc;
use saikuro_router::provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem};
use saikuro_router::router::{InvocationRouter, RouterConfig};
use core::time::Duration;

pub fn register(suite: &mut TestSuite) {
    suite.register("router::call_returns_response", call_returns_response);
    suite.register("router::cast_returns_ok_empty", cast_returns_ok_empty);
    suite.register(
        "router::call_unknown_namespace_no_provider",
        call_unknown_namespace_no_provider,
    );
    suite.register(
        "router::call_dropped_provider_unavailable",
        call_dropped_provider_unavailable,
    );
    suite.register(
        "router::multiple_sequential_calls",
        multiple_sequential_calls,
    );
    suite.register(
        "router::cast_unknown_namespace_ok",
        cast_unknown_namespace_ok,
    );
    suite.register(
        "router::call_null_target_malformed",
        call_null_target_malformed,
    );
    suite.register(
        "router::call_times_out_when_provider_does_not_respond",
        call_times_out_when_provider_does_not_respond,
    );
    suite.register(
        "router::concurrent_calls_all_succeed",
        concurrent_calls_all_succeed,
    );
}

/// A responder that never answers (used to trigger timeouts).
///
/// It holds the received work items (including their `response_tx` channels)
/// alive so the router blocks on the oneshot and eventually times out rather
/// than seeing a dropped sender (which would produce ProviderUnavailable).
fn spawn_silent_responder(
    mut work_rx: mpsc::Receiver<ProviderWorkItem>,
) -> saikuro_exec::JoinHandle<()> {
    saikuro_exec::spawn(async move {
        let mut held = crate::Vec::new();
        while let Some(item) = work_rx.recv().await {
            // Keep the item alive so response_tx is not dropped.
            held.push(item);
        }
        // held is dropped when the task ends, but by then the test is over.
        drop(held);
    })
}

fn call_returns_response() -> Result<(), &'static str> {
    crate::block_on(async {
        let (work_tx, work_rx) = mpsc::channel::<ProviderWorkItem>(common::capacity(64));
        let handle = ProviderHandle::new("math-provider", vec!["math".into()], work_tx);
        let registry = ProviderRegistry::new();
        registry.register(handle).await;
        common::spawn_responder(work_rx, Value::Int(42));

        let router = InvocationRouter::with_providers(registry);
        let env =
            Envelope::call("math.add", vec![Value::Int(1), Value::Int(2)]).map_err(|_| "create")?;
        let resp = router.dispatch(env).await;
        assert!(resp.ok);
        assert_eq!(resp.result, Some(Value::Int(42)));
        Ok(())
    })
}

fn cast_returns_ok_empty() -> Result<(), &'static str> {
    crate::block_on(async {
        let (work_tx, mut work_rx) = mpsc::channel::<ProviderWorkItem>(common::capacity(64));
        let handle = ProviderHandle::new("logger", vec!["logger".into()], work_tx);
        let registry = ProviderRegistry::new();
        registry.register(handle).await;
        saikuro_exec::spawn(async move { while (work_rx.recv().await).is_some() {} });

        let router = InvocationRouter::with_providers(registry);
        let env = Envelope::cast("logger.info", vec![Value::String("hello".into())])
            .map_err(|_| "create")?;
        let resp = router.dispatch(env).await;
        assert!(resp.ok);
        assert!(resp.result.is_none());
        Ok(())
    })
}

fn call_unknown_namespace_no_provider() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = ProviderRegistry::new();
        let router = InvocationRouter::with_providers(registry);
        let env = Envelope::call("nonexistent.fn", vec![]).map_err(|_| "create")?;
        let resp = router.dispatch(env).await;
        assert!(!resp.ok);
        assert_eq!(
            resp.error.as_ref().map(|e| e.code.clone()),
            Some(ErrorCode::NoProvider)
        );
        Ok(())
    })
}

fn call_dropped_provider_unavailable() -> Result<(), &'static str> {
    crate::block_on(async {
        let (work_tx, work_rx) =
            mpsc::channel::<ProviderWorkItem>(saikuro_exec::ChannelCapacity::MIN);
        let handle = ProviderHandle::new("gone", vec!["svc".into()], work_tx);
        let registry = ProviderRegistry::new();
        registry.register(handle).await;
        drop(work_rx);
        let router = InvocationRouter::with_providers(registry);
        let env = Envelope::call("svc.op", vec![]).map_err(|_| "create")?;
        let resp = router.dispatch(env).await;
        assert!(!resp.ok);
        let err = resp.error.as_ref().map(|e| e.code.clone()).unwrap();
        assert!(
            err == ErrorCode::ProviderUnavailable || err == ErrorCode::NoProvider,
            "expected ProviderUnavailable or NoProvider, got {:?}",
            err
        );
        Ok(())
    })
}

fn multiple_sequential_calls() -> Result<(), &'static str> {
    crate::block_on(async {
        let (work_tx, work_rx) = mpsc::channel::<ProviderWorkItem>(common::capacity(64));
        let handle = ProviderHandle::new("counter", vec!["counter".into()], work_tx);
        let registry = ProviderRegistry::new();
        registry.register(handle).await;
        common::spawn_responder(work_rx, Value::Bool(true));

        let router = InvocationRouter::with_providers(registry);
        for _ in 0..5 {
            let env = Envelope::call("counter.inc", vec![]).map_err(|_| "create")?;
            let resp = router.dispatch(env).await;
            assert!(resp.ok);
        }
        Ok(())
    })
}

fn cast_unknown_namespace_ok() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = ProviderRegistry::new();
        let router = InvocationRouter::with_providers(registry);
        let env = Envelope::cast("missing.fn", vec![]).map_err(|_| "create")?;
        let resp = router.dispatch(env).await;
        let _ = resp;
        Ok(())
    })
}

fn call_null_target_malformed() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = ProviderRegistry::new();
        let router = InvocationRouter::with_providers(registry);
        let env = Envelope::call("nodothere", vec![]).map_err(|_| "create")?;
        let resp = router.dispatch(env).await;
        assert!(!resp.ok);
        let err = resp.error.as_ref().unwrap();
        assert!(
            err.code == ErrorCode::MalformedTarget
                || err.code == ErrorCode::MalformedEnvelope
                || err.code == ErrorCode::NoProvider,
            "unexpected {:?}",
            err.code
        );
        Ok(())
    })
}

fn call_times_out_when_provider_does_not_respond() -> Result<(), &'static str> {
    crate::block_on(async {
        let (registry, work_rx) = common::make_provider("slow").await;
        let _silent = spawn_silent_responder(work_rx);

        let config = RouterConfig {
            call_timeout: Duration::from_millis(50),
            ..Default::default()
        };
        let router = InvocationRouter::new(registry, config);

        let env = Envelope::call("slow.fn", vec![]).map_err(|_| "create")?;
        let resp = router.dispatch(env).await;

        assert!(!resp.ok);
        assert_eq!(
            resp.error.as_ref().map(|e| e.code.clone()),
            Some(ErrorCode::Timeout)
        );
        Ok(())
    })
}

fn concurrent_calls_all_succeed() -> Result<(), &'static str> {
    crate::block_on(async {
        let (registry, work_rx) = common::make_provider("parallel").await;
        let _responder = common::spawn_responder(work_rx, Value::Int(0));

        let router = InvocationRouter::with_providers(registry);

        let mut handles = vec![];
        for _ in 0..20 {
            let r = router.clone();
            handles.push(saikuro_exec::spawn(async move {
                let env = Envelope::call("parallel.op", vec![]).map_err(|_| "create")?;
                Ok::<_, &'static str>(r.dispatch(env).await)
            }));
        }

        for h in handles {
            let resp = h.await.map_err(|_| "task panicked")??;
            assert!(resp.ok, "concurrent call should succeed");
        }
        Ok(())
    })
}
