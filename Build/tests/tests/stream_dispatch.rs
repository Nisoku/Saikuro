//! Stream dispatch tests.

use saikuro_core::{
    envelope::{Envelope, StreamControl},
    error::ErrorCode,
    invocation::InvocationId,
    value::Value,
    ResponseEnvelope,
};
use saikuro_router::provider::ProviderRegistry;
use saikuro_router::router::InvocationRouter;

mod common;

//  Tests

#[test]
fn stream_open_returns_ok_empty() {
    saikuro_exec::block_on(async {
        let (registry, mut work_rx) = common::make_provider("events");

        // Consume work items (provider side).
        saikuro_exec::spawn(async move { while (work_rx.recv().await).is_some() {} });

        let router = InvocationRouter::with_providers(registry);
        let env = Envelope::stream_open("events.subscribe", vec![Value::String("topic".into())]);
        let resp = router.dispatch(env).await;

        assert!(resp.ok, "stream open should return ok");
        assert!(resp.result.is_none(), "stream open returns empty result");
        assert!(resp.stream_control.is_none());
    })
}

#[test]
fn route_stream_item_delivers_to_state() {
    saikuro_exec::block_on(async {
        let (registry, mut work_rx) = common::make_provider("data");

        // The provider will send items back via route_stream_item.
        let router = InvocationRouter::with_providers(registry);

        // Open the stream to register it in the state store.
        let open_env = Envelope::stream_open("data.feed", vec![]);
        let stream_id = open_env.id;

        saikuro_exec::spawn(async move { while (work_rx.recv().await).is_some() {} });

        let open_resp = router.dispatch(open_env).await;
        assert!(open_resp.ok);

        // Route an item to the stream.
        let item = ResponseEnvelope::stream_item(stream_id, 0, Value::Int(100));
        let result = router.route_stream_item(item).await;
        assert!(result.is_ok(), "routing a valid stream item should succeed");
    })
}

#[test]
fn route_stream_end_removes_state() {
    saikuro_exec::block_on(async {
        let (registry, mut work_rx) = common::make_provider("fin");

        let router = InvocationRouter::with_providers(registry);
        let open_env = Envelope::stream_open("fin.feed", vec![]);
        let stream_id = open_env.id;

        saikuro_exec::spawn(async move { while (work_rx.recv().await).is_some() {} });

        router.dispatch(open_env).await;

        // Route the end sentinel.
        let end = ResponseEnvelope::stream_end(stream_id, 0);
        let result = router.route_stream_item(end).await;
        assert!(result.is_ok());

        // After EOS the state entry is removed:  routing another item should fail.
        let extra = ResponseEnvelope::stream_item(stream_id, 1, Value::Null);
        let err = router.route_stream_item(extra).await;
        assert!(err.is_err(), "routing to removed stream should fail");
    })
}

#[test]
fn route_to_unknown_stream_returns_error() {
    saikuro_exec::block_on(async {
        let registry = ProviderRegistry::new();
        let router = InvocationRouter::with_providers(registry);

        let phantom_id = InvocationId::new();
        let item = ResponseEnvelope::stream_item(phantom_id, 0, Value::Null);
        let err = router.route_stream_item(item).await;
        assert!(err.is_err(), "routing to non-existent stream should fail");
    })
}

#[test]
fn stream_open_to_unknown_namespace_returns_no_provider() {
    saikuro_exec::block_on(async {
        let registry = ProviderRegistry::new();
        let router = InvocationRouter::with_providers(registry);

        let env = Envelope::stream_open("ghost.feed", vec![]);
        let resp = router.dispatch(env).await;

        assert!(!resp.ok);
        let err = resp.error.unwrap();
        assert_eq!(err.code, ErrorCode::NoProvider);
    })
}

#[test]
fn multiple_streams_are_independent() {
    saikuro_exec::block_on(async {
        let (registry, mut work_rx) = common::make_provider("multi");
        let router = InvocationRouter::with_providers(registry);

        saikuro_exec::spawn(async move { while (work_rx.recv().await).is_some() {} });

        // Open two streams.
        let env1 = Envelope::stream_open("multi.s1", vec![]);
        let env2 = Envelope::stream_open("multi.s2", vec![]);
        let id1 = env1.id;
        let id2 = env2.id;

        router.dispatch(env1).await;
        router.dispatch(env2).await;

        // Route an item to stream 1.
        let item1 = ResponseEnvelope::stream_item(id1, 0, Value::Int(1));
        assert!(router.route_stream_item(item1).await.is_ok());

        // Route an item to stream 2.
        let item2 = ResponseEnvelope::stream_item(id2, 0, Value::Int(2));
        assert!(router.route_stream_item(item2).await.is_ok());

        // Close stream 1; stream 2 is still alive.
        let end1 = ResponseEnvelope::stream_end(id1, 1);
        router.route_stream_item(end1).await.ok();

        let item2b = ResponseEnvelope::stream_item(id2, 1, Value::Int(99));
        assert!(
            router.route_stream_item(item2b).await.is_ok(),
            "stream 2 should still accept items after stream 1 closes"
        );
    })
}

#[test]
fn out_of_order_item_is_dropped_not_panicked() {
    saikuro_exec::block_on(async {
        let (registry, mut work_rx) = common::make_provider("ooo");
        let router = InvocationRouter::with_providers(registry);

        saikuro_exec::spawn(async move { while (work_rx.recv().await).is_some() {} });

        let env = Envelope::stream_open("ooo.feed", vec![]);
        let id = env.id;
        router.dispatch(env).await;

        // First item (seq=0) is fine.
        let item0 = ResponseEnvelope::stream_item(id, 0, Value::Int(0));
        router.route_stream_item(item0).await.ok();

        // Skip seq=1 and send seq=5:  should not panic, just log a warning.
        let item5 = ResponseEnvelope::stream_item(id, 5, Value::Int(5));
        // The router should handle this gracefully (either drop or accept).
        let _ = router.route_stream_item(item5).await;
    })
}

#[test]
fn stream_abort_control_removes_state() {
    saikuro_exec::block_on(async {
        let (registry, mut work_rx) = common::make_provider("abort");
        let router = InvocationRouter::with_providers(registry);

        saikuro_exec::spawn(async move { while (work_rx.recv().await).is_some() {} });

        let env = Envelope::stream_open("abort.feed", vec![]);
        let id = env.id;
        router.dispatch(env).await;

        // Send an Abort control frame.
        let abort = ResponseEnvelope {
            id,
            ok: false,
            result: None,
            error: None,
            seq: Some(0),
            stream_control: Some(StreamControl::Abort),
        };
        let result = router.route_stream_item(abort).await;
        // Should succeed in routing the abort frame.
        assert!(result.is_ok());

        // Subsequent routing should fail:  state has been removed.
        let extra = ResponseEnvelope::stream_item(id, 1, Value::Null);
        assert!(router.route_stream_item(extra).await.is_err());
    })
}
