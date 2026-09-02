//! Batch dispatch integration tests

use saikuro_core::{
    envelope::{Envelope, InvocationType},
    ResponseEnvelope,
};
use saikuro_event::{ErrorCode, Value};
use saikuro_exec::mpsc;
use saikuro_router::{
    provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem},
    router::InvocationRouter,
};

//  Helpers

async fn register_echo_provider(registry: &ProviderRegistry, namespace: &str, response: Value) {
    let (work_tx, work_rx) = mpsc::channel::<ProviderWorkItem>(
        saikuro_exec::ChannelCapacity::try_from(64).expect("64 is a valid channel capacity"),
    );
    let handle = ProviderHandle::new(
        format!("{namespace}-provider"),
        vec![namespace.to_owned()],
        work_tx,
    );
    registry.register(handle).await;

    // Spawn a background responder.
    saikuro_exec::spawn({
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
}

//  Tests


#[test]
fn batch_with_no_items_field_returns_malformed() {
    saikuro_exec::block_on(async {
        let registry = ProviderRegistry::new();
        let router = InvocationRouter::with_providers(registry);

        let mut batch = Envelope::call("", vec![]).expect("entropy available");
        batch.invocation_type = InvocationType::Batch;
        batch.target = String::new();
        batch.batch_items = None; // explicitly absent

        let resp = router.dispatch(batch).await;
        assert!(!resp.ok);
        let err = resp.error.unwrap();
        assert_eq!(err.code, ErrorCode::MalformedEnvelope);
    })
}

#[test]
fn batch_items_targeting_different_namespaces() {
    saikuro_exec::block_on(async {
        let registry = ProviderRegistry::new();
        register_echo_provider(&registry, "ns_a", Value::Bool(true)).await;
        register_echo_provider(&registry, "ns_b", Value::Int(0)).await;

        let router = InvocationRouter::with_providers(registry);

        let items = vec![
            Envelope::call("ns_a.fn", vec![]).expect("entropy available"),
            Envelope::call("ns_b.fn", vec![]).expect("entropy available"),
        ];

        let mut batch = Envelope::call("", vec![]).expect("entropy available");
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
    })
}

#[test]
fn batch_item_to_unknown_namespace_returns_null_in_result() {
    saikuro_exec::block_on(async {
        // Per our router implementation, failed batch items produce Null in the
        // results array (not an error on the whole batch).
        let registry = ProviderRegistry::new();
        register_echo_provider(&registry, "known", Value::Int(1)).await;

        let router = InvocationRouter::with_providers(registry);

        let items = vec![
            Envelope::call("known.fn", vec![]).expect("entropy available"),
            Envelope::call("ghost.fn", vec![]).expect("entropy available"), // no provider for this
        ];

        let mut batch = Envelope::call("", vec![]).expect("entropy available");
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
    })
}
