//! Batch dispatch integration tests

use saikuro_core::{
    envelope::{Envelope, InvocationType},
    error::ErrorCode,
    value::Value,
    ResponseEnvelope,
};
use saikuro_router::{
    provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem},
    router::InvocationRouter,
};
use tokio::sync::mpsc;

//  Helpers

fn register_echo_provider(
    registry: &ProviderRegistry,
    namespace: &str,
    response: Value,
) -> mpsc::Receiver<ProviderWorkItem> {
    let (work_tx, work_rx) = mpsc::channel::<ProviderWorkItem>(64);
    let handle = ProviderHandle::new(
        format!("{namespace}-provider"),
        vec![namespace.to_owned()],
        work_tx,
    );
    registry.register(handle);

    // Spawn a background responder.
    tokio::spawn({
        let response = response.clone();
        async move {
            let mut rx = work_rx;
            while let Some(item) = rx.recv().await {
                if let Some(tx) = item.response_tx {
                    let _ = tx.send(ResponseEnvelope::ok(item.envelope.id, response.clone()));
                }
            }
        }
    });

    // Return a dummy receiver:  we've already handed ownership to the task.
    // We need to return something; use a channel that is immediately dropped.
    let (_dummy_tx, dummy_rx) = mpsc::channel(1);
    dummy_rx
}

//  Tests

#[tokio::test]
async fn batch_with_single_item_succeeds() {
    let registry = ProviderRegistry::new();
    register_echo_provider(&registry, "math", Value::Int(7));

    let router = InvocationRouter::with_providers(registry);

    let item = Envelope::call("math.add", vec![Value::Int(3), Value::Int(4)]);
    let mut batch = Envelope::call("", vec![]);
    batch.invocation_type = InvocationType::Batch;
    batch.target = String::new();
    batch.batch_items = Some(vec![item]);

    let resp = router.dispatch(batch).await;
    assert!(resp.ok, "batch should succeed");

    let Value::Array(results) = resp.result.unwrap() else {
        panic!("expected Array result");
    };
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Value::Int(7));
}

#[tokio::test]
async fn batch_with_multiple_items_returns_all_results() {
    let registry = ProviderRegistry::new();
    register_echo_provider(&registry, "svc", Value::Int(42));

    let router = InvocationRouter::with_providers(registry);

    let items: Vec<Envelope> = (0..5)
        .map(|i| Envelope::call("svc.op", vec![Value::Int(i)]))
        .collect();

    let mut batch = Envelope::call("", vec![]);
    batch.invocation_type = InvocationType::Batch;
    batch.target = String::new();
    batch.batch_items = Some(items);

    let resp = router.dispatch(batch).await;
    assert!(resp.ok);

    let Value::Array(results) = resp.result.unwrap() else {
        panic!("expected Array");
    };
    assert_eq!(results.len(), 5);
    for r in &results {
        assert_eq!(*r, Value::Int(42));
    }
}

#[tokio::test]
async fn batch_with_no_items_field_returns_malformed() {
    let registry = ProviderRegistry::new();
    let router = InvocationRouter::with_providers(registry);

    let mut batch = Envelope::call("", vec![]);
    batch.invocation_type = InvocationType::Batch;
    batch.target = String::new();
    batch.batch_items = None; // explicitly absent

    let resp = router.dispatch(batch).await;
    assert!(!resp.ok);
    let err = resp.error.unwrap();
    assert_eq!(err.code, ErrorCode::MalformedEnvelope);
}

#[tokio::test]
async fn batch_items_targeting_different_namespaces() {
    let registry = ProviderRegistry::new();
    register_echo_provider(&registry, "ns_a", Value::Bool(true));
    register_echo_provider(&registry, "ns_b", Value::Int(0));

    let router = InvocationRouter::with_providers(registry);

    let items = vec![
        Envelope::call("ns_a.fn", vec![]),
        Envelope::call("ns_b.fn", vec![]),
    ];

    let mut batch = Envelope::call("", vec![]);
    batch.invocation_type = InvocationType::Batch;
    batch.target = String::new();
    batch.batch_items = Some(items);

    let resp = router.dispatch(batch).await;
    assert!(resp.ok);

    let Value::Array(results) = resp.result.unwrap() else {
        panic!("expected Array");
    };
    assert_eq!(results.len(), 2);
    // Results are in order: first ns_a (Bool(true)), then ns_b (Int(0)).
    assert_eq!(results[0], Value::Bool(true));
    assert_eq!(results[1], Value::Int(0));
}

#[tokio::test]
async fn batch_item_to_unknown_namespace_returns_null_in_result() {
    // Per our router implementation, failed batch items produce Null in the
    // results array (not an error on the whole batch).
    let registry = ProviderRegistry::new();
    register_echo_provider(&registry, "known", Value::Int(1));

    let router = InvocationRouter::with_providers(registry);

    let items = vec![
        Envelope::call("known.fn", vec![]),
        Envelope::call("ghost.fn", vec![]), // no provider for this
    ];

    let mut batch = Envelope::call("", vec![]);
    batch.invocation_type = InvocationType::Batch;
    batch.target = String::new();
    batch.batch_items = Some(items);

    let resp = router.dispatch(batch).await;
    assert!(resp.ok, "batch itself should still succeed");

    let Value::Array(results) = resp.result.unwrap() else {
        panic!("expected Array");
    };
    assert_eq!(results.len(), 2);
    assert_eq!(results[0], Value::Int(1));
    assert_eq!(results[1], Value::Null, "failed item should be Null");
}

#[tokio::test]
async fn batch_result_is_ordered_array() {
    let registry = ProviderRegistry::new();

    // Provider that echos back the first integer argument.
    let (work_tx, mut work_rx) = mpsc::channel::<ProviderWorkItem>(64);
    let handle = ProviderHandle::new("ordered", vec!["ord".to_owned()], work_tx);
    registry.register(handle);

    tokio::spawn(async move {
        while let Some(item) = work_rx.recv().await {
            if let Some(tx) = item.response_tx {
                let val = item.envelope.args.into_iter().next().unwrap_or(Value::Null);
                let _ = tx.send(ResponseEnvelope::ok(item.envelope.id, val));
            }
        }
    });

    let router = InvocationRouter::with_providers(registry);

    let items: Vec<Envelope> = vec![10i64, 20, 30, 40]
        .into_iter()
        .map(|n| Envelope::call("ord.fn", vec![Value::Int(n)]))
        .collect();

    let mut batch = Envelope::call("", vec![]);
    batch.invocation_type = InvocationType::Batch;
    batch.target = String::new();
    batch.batch_items = Some(items);

    let resp = router.dispatch(batch).await;
    assert!(resp.ok);

    let Value::Array(results) = resp.result.unwrap() else {
        panic!("expected Array");
    };
    assert_eq!(
        results,
        vec![
            Value::Int(10),
            Value::Int(20),
            Value::Int(30),
            Value::Int(40)
        ]
    );
}
