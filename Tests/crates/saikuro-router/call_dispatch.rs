//! Call and cast dispatch integration tests

use crate::common;
use saikuro_core::{envelope::Envelope, ResponseEnvelope};
use saikuro_event::{ErrorCode, Value};
use saikuro_exec::mpsc;
use saikuro_router::{
    provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem},
    router::{InvocationRouter, RouterConfig},
};
use std::time::Duration;

//  Helpers
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
fn call_times_out_when_provider_does_not_respond() {
    saikuro_exec::block_on(async {
        let (registry, work_rx) = common::make_provider("slow").await;
        let _silent = spawn_silent_responder(work_rx);

        let config = RouterConfig {
            call_timeout: Duration::from_millis(50),
            ..Default::default()
        };
        let router = InvocationRouter::new(registry, config);

        let env = Envelope::call("slow.fn", vec![]).expect("entropy available");
        let resp = router.dispatch(env).await;

        assert!(!resp.ok);
        let err = resp.error.expect("error detail");
        assert_eq!(err.code, ErrorCode::Timeout);
    })
}


#[test]
fn concurrent_calls_all_succeed() {
    saikuro_exec::block_on(async {
        let (registry, work_rx) = common::make_provider("parallel").await;
        let _responder = spawn_responder(work_rx, Value::Int(0));

        let router = InvocationRouter::with_providers(registry);

        let mut handles = vec![];
        for _ in 0..20 {
            let r = router.clone();
            handles.push(saikuro_exec::spawn(async move {
                let env = Envelope::call("parallel.op", vec![]).expect("entropy available");
                r.dispatch(env).await
            }));
        }

        for h in handles {
            let resp = h.await.expect("task panicked");
            assert!(resp.ok, "concurrent call should succeed");
        }
    })
}
