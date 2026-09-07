
use crate::common;
use crate::shared_test;
use crate::TestSuite;
use saikuro_core::envelope::{Envelope, InvocationType};
use saikuro_core::ResponseEnvelope;
use saikuro_event::{ErrorCode, Value};
use saikuro_exec::mpsc;
use saikuro_router::provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem};
use saikuro_router::router::InvocationRouter;

pub fn register(suite: &mut TestSuite) {
    shared_test!(suite, "router::batch_single_item", batch_single_item);
    shared_test!(suite, "router::batch_multiple_items", batch_multiple_items);
    shared_test!(suite, "router::batch_result_ordered", batch_result_ordered);
    shared_test!(suite,
        "router::batch_no_items_field_malformed",
        batch_no_items_field_malformed,
    );
    shared_test!(suite,
        "router::batch_items_different_namespaces",
        batch_items_different_namespaces,
    );
    shared_test!(suite,
        "router::batch_item_unknown_namespace_null_result",
        batch_item_unknown_namespace_null_result,
    );
}

async fn register_echo_provider(
    registry: &ProviderRegistry,
    namespace: &str,
    response: Value,
) {
    let (work_tx, work_rx) = mpsc::channel::<ProviderWorkItem>(common::capacity(64));
    let handle = ProviderHandle::new(
        format!("{namespace}-provider"),
        vec![namespace.into()],
        work_tx,
    );
    registry.register(handle).await;

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

fn batch_single_item() -> Result<(), &'static str> {
    crate::block_on(async {
        let (work_tx, work_rx) = mpsc::channel::<ProviderWorkItem>(common::capacity(64));
        let handle = ProviderHandle::new("math", vec!["math".into()], work_tx);
        let registry = ProviderRegistry::new();
        registry.register(handle).await;
        common::spawn_responder(work_rx, Value::Int(10));

        let router = InvocationRouter::with_providers(registry);
        let inner = Envelope::call("math.add", vec![]).map_err(|_| "inner")?;
        let mut env = Envelope::call("$saikuro.batch", vec![]).map_err(|_| "batch")?;
        env.invocation_type = InvocationType::Batch;
        env.batch_items = Some(vec![inner]);
        let resp = router.dispatch(env).await;
        assert!(resp.ok);
        Ok(())
    })
}

fn batch_no_items_field_malformed() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = ProviderRegistry::new();
        let router = InvocationRouter::with_providers(registry);

        let mut batch = Envelope::call("", vec![]).map_err(|_| "create batch")?;
        batch.invocation_type = InvocationType::Batch;
        batch.target = crate::String::new();
        batch.batch_items = None;

        let resp = router.dispatch(batch).await;
        assert!(!resp.ok);
        assert_eq!(
            resp.error.as_ref().map(|e| e.code.clone()),
            Some(ErrorCode::MalformedEnvelope)
        );
        Ok(())
    })
}

fn batch_items_different_namespaces() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = ProviderRegistry::new();
        register_echo_provider(&registry, "ns_a", Value::Bool(true)).await;
        register_echo_provider(&registry, "ns_b", Value::Int(0)).await;

        let router = InvocationRouter::with_providers(registry);

        let items = vec![
            Envelope::call("ns_a.fn", vec![]).map_err(|_| "ns_a item")?,
            Envelope::call("ns_b.fn", vec![]).map_err(|_| "ns_b item")?,
        ];

        let mut batch = Envelope::call("", vec![]).map_err(|_| "create batch")?;
        batch.invocation_type = InvocationType::Batch;
        batch.target = crate::String::new();
        batch.batch_items = Some(items);

        let resp = router.dispatch(batch).await;
        assert!(resp.ok);

        let results = match resp.result {
            Some(Value::Array(results)) => results,
            _ => return Err("expected Array result"),
        };
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], Value::Bool(true));
        assert_eq!(results[1], Value::Int(0));
        Ok(())
    })
}

fn batch_item_unknown_namespace_null_result() -> Result<(), &'static str> {
    crate::block_on(async {
        // Per router semantics, failed batch items produce Null in the results
        // array (not an error for the whole batch).
        let registry = ProviderRegistry::new();
        register_echo_provider(&registry, "known", Value::Int(1)).await;

        let router = InvocationRouter::with_providers(registry);

        let items = vec![
            Envelope::call("known.fn", vec![]).map_err(|_| "known item")?,
            Envelope::call("ghost.fn", vec![]).map_err(|_| "ghost item")?,
        ];

        let mut batch = Envelope::call("", vec![]).map_err(|_| "create batch")?;
        batch.invocation_type = InvocationType::Batch;
        batch.target = crate::String::new();
        batch.batch_items = Some(items);

        let resp = router.dispatch(batch).await;
        assert!(resp.ok, "batch itself should still succeed");

        let results = match resp.result {
            Some(Value::Array(results)) => results,
            _ => return Err("expected Array result"),
        };
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], Value::Int(1));
        assert_eq!(results[1], Value::Null, "failed item should be Null");
        Ok(())
    })
}

fn batch_multiple_items() -> Result<(), &'static str> {
    crate::block_on(async {
        let (work_tx, work_rx) = mpsc::channel::<ProviderWorkItem>(common::capacity(64));
        let handle = ProviderHandle::new("svc", vec!["svc".into()], work_tx);
        let registry = ProviderRegistry::new();
        registry.register(handle).await;
        common::spawn_responder(work_rx, Value::Int(1));

        let router = InvocationRouter::with_providers(registry);
        let i1 = Envelope::call("svc.a", vec![]).map_err(|_| "i1")?;
        let i2 = Envelope::call("svc.b", vec![]).map_err(|_| "i2")?;
        let i3 = Envelope::call("svc.c", vec![]).map_err(|_| "i3")?;
        let mut env = Envelope::call("$saikuro.batch", vec![]).map_err(|_| "batch")?;
        env.invocation_type = InvocationType::Batch;
        env.batch_items = Some(vec![i1, i2, i3]);
        let resp = router.dispatch(env).await;
        assert!(resp.ok);
        if let Some(Value::Array(items)) = &resp.result {
            assert_eq!(items.len(), 3);
        } else {
            return Err("expected array result");
        }
        Ok(())
    })
}

fn batch_result_ordered() -> Result<(), &'static str> {
    crate::block_on(async {
        let (work_tx, work_rx) = mpsc::channel::<ProviderWorkItem>(common::capacity(64));
        let handle = ProviderHandle::new("svc", vec!["svc".into()], work_tx);
        let registry = ProviderRegistry::new();
        registry.register(handle).await;
        common::spawn_responder(work_rx, Value::Int(0));

        let router = InvocationRouter::with_providers(registry);
        let i1 = Envelope::call("svc.a", vec![]).map_err(|_| "i1")?;
        let i2 = Envelope::call("svc.b", vec![]).map_err(|_| "i2")?;
        let mut env = Envelope::call("$saikuro.batch", vec![]).map_err(|_| "batch")?;
        env.invocation_type = InvocationType::Batch;
        env.batch_items = Some(vec![i1, i2]);
        let resp = router.dispatch(env).await;
        assert!(resp.ok);
        Ok(())
    })
}
