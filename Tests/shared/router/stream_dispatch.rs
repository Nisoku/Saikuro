//! Stream dispatch integration tests.

use crate::check_test;
use crate::common;
use crate::TestSuite;
use core::task::Poll;
use futures::{pin_mut, poll};
use saikuro_core::{
    envelope::{Envelope, StreamControl},
    invocation::InvocationId,
    ResponseEnvelope,
};
use saikuro_event::{ErrorCode, Value};
use saikuro_router::{
    provider::ProviderRegistry,
    router::InvocationRouter,
    stream_state::{DeliveryOutcome, StreamState},
};

pub fn register(suite: &mut TestSuite) {
    suite.register(
        "router::stream_open_returns_ok_empty",
        stream_open_returns_ok_empty,
    );
    suite.register(
        "router::route_stream_item_delivers_to_state",
        route_stream_item_delivers_to_state,
    );
    suite.register(
        "router::route_stream_end_removes_state",
        route_stream_end_removes_state,
    );
    suite.register(
        "router::route_to_unknown_stream_returns_error",
        route_to_unknown_stream_returns_error,
    );
    suite.register(
        "router::stream_open_to_unknown_namespace_returns_no_provider",
        stream_open_to_unknown_namespace_returns_no_provider,
    );
    suite.register(
        "router::multiple_streams_are_independent",
        multiple_streams_are_independent,
    );
    suite.register(
        "router::out_of_order_item_is_dropped_not_panicked",
        out_of_order_item_is_dropped_not_panicked,
    );
    suite.register(
        "router::stream_abort_control_removes_state",
        stream_abort_control_removes_state,
    );
    suite.register(
        "router::concurrent_stream_delivery_preserves_order_and_terminal_closure",
        concurrent_stream_delivery_preserves_order_and_terminal_closure,
    );
}

fn stream_open_returns_ok_empty() -> Result<(), &'static str> {
    crate::block_on(async {
        let (registry, mut work_rx) = common::make_provider("events").await;
        saikuro_exec::spawn(async move { while work_rx.recv().await.is_some() {} });

        let router = InvocationRouter::with_providers(registry);
        let env = Envelope::stream_open(
            "events.subscribe",
            crate::vec![Value::String("topic".into())],
        )
        .map_err(|_| "create")?;
        let resp = router.dispatch(env).await;

        check_test!(resp.ok, "stream open should return ok");
        check_test!(resp.result.is_none(), "stream open returns empty result");
        check_test!(resp.stream_control.is_none(), "no stream control on open");
        Ok(())
    })
}

fn route_stream_item_delivers_to_state() -> Result<(), &'static str> {
    crate::block_on(async {
        let (registry, mut work_rx) = common::make_provider("data").await;
        let router = InvocationRouter::with_providers(registry);

        // Open the stream to register it in the state store.
        let open_env = Envelope::stream_open("data.feed", crate::vec![])
            .map_err(|_| "create")?;
        let stream_id = open_env.id;

        saikuro_exec::spawn(async move { while work_rx.recv().await.is_some() {} });

        let open_resp = router.dispatch(open_env).await;
        check_test!(open_resp.ok, "stream must open");

        // Route an item to the stream.
        let item = ResponseEnvelope::stream_item(stream_id, 0, Value::Int(100));
        check_test!(
            router.route_stream_item(item).await.is_ok(),
            "routing a valid stream item should succeed"
        );
        Ok(())
    })
}

fn route_stream_end_removes_state() -> Result<(), &'static str> {
    crate::block_on(async {
        let (registry, mut work_rx) = common::make_provider("fin").await;
        let router = InvocationRouter::with_providers(registry);
        let open_env = Envelope::stream_open("fin.feed", crate::vec![]).map_err(|_| "create")?;
        let stream_id = open_env.id;

        saikuro_exec::spawn(async move { while work_rx.recv().await.is_some() {} });

        router.dispatch(open_env).await;

        // Route the end sentinel.
        let end = ResponseEnvelope::stream_end(stream_id, 0);
        let result = router.route_stream_item(end).await;
        check_test!(result.is_ok(), "end sentinel must route");

        // After EOS the state entry is removed: routing another item fails.
        let extra = ResponseEnvelope::stream_item(stream_id, 1, Value::Null);
        check_test!(
            router.route_stream_item(extra).await.is_err(),
            "routing to removed stream should fail"
        );
        Ok(())
    })
}

fn route_to_unknown_stream_returns_error() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = ProviderRegistry::new();
        let router = InvocationRouter::with_providers(registry);

        let phantom_id = InvocationId::new().map_err(|_| "entropy")?;
        let item = ResponseEnvelope::stream_item(phantom_id, 0, Value::Null);
        check_test!(
            router.route_stream_item(item).await.is_err(),
            "routing to non-existent stream should fail"
        );
        Ok(())
    })
}

fn stream_open_to_unknown_namespace_returns_no_provider() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = ProviderRegistry::new();
        let router = InvocationRouter::with_providers(registry);

        let env = Envelope::stream_open("ghost.feed", crate::vec![]).map_err(|_| "create")?;
        let resp = router.dispatch(env).await;

        check_test!(!resp.ok, "unknown namespace must fail");
        assert_eq!(
            resp.error.as_ref().map(|e| e.code.clone()),
            Some(ErrorCode::NoProvider)
        );
        Ok(())
    })
}

fn multiple_streams_are_independent() -> Result<(), &'static str> {
    crate::block_on(async {
        let (registry, mut work_rx) = common::make_provider("multi").await;
        let router = InvocationRouter::with_providers(registry);

        saikuro_exec::spawn(async move { while work_rx.recv().await.is_some() {} });

        // Open two streams.
        let env1 = Envelope::stream_open("multi.s1", crate::vec![]).map_err(|_| "create")?;
        let env2 = Envelope::stream_open("multi.s2", crate::vec![]).map_err(|_| "create")?;
        let id1 = env1.id;
        let id2 = env2.id;

        router.dispatch(env1).await;
        router.dispatch(env2).await;

        // Route an item to stream 1.
        let item1 = ResponseEnvelope::stream_item(id1, 0, Value::Int(1));
        check_test!(router.route_stream_item(item1).await.is_ok(), "s1 item");

        // Route an item to stream 2.
        let item2 = ResponseEnvelope::stream_item(id2, 0, Value::Int(2));
        check_test!(router.route_stream_item(item2).await.is_ok(), "s2 item");

        // Close stream 1; stream 2 is still alive.
        let end1 = ResponseEnvelope::stream_end(id1, 1);
        router.route_stream_item(end1).await.ok();

        let item2b = ResponseEnvelope::stream_item(id2, 1, Value::Int(99));
        check_test!(
            router.route_stream_item(item2b).await.is_ok(),
            "stream 2 should still accept items after stream 1 closes"
        );
        Ok(())
    })
}

fn out_of_order_item_is_dropped_not_panicked() -> Result<(), &'static str> {
    crate::block_on(async {
        let (registry, mut work_rx) = common::make_provider("ooo").await;
        let router = InvocationRouter::with_providers(registry);

        saikuro_exec::spawn(async move { while work_rx.recv().await.is_some() {} });

        let env = Envelope::stream_open("ooo.feed", crate::vec![]).map_err(|_| "create")?;
        let id = env.id;
        router.dispatch(env).await;

        // First item (seq=0) is fine.
        let item0 = ResponseEnvelope::stream_item(id, 0, Value::Int(0));
        router.route_stream_item(item0).await.ok();

        // Skip seq=1 and send seq=5: should not panic, just log a warning.
        let item5 = ResponseEnvelope::stream_item(id, 5, Value::Int(5));
        let _ = router.route_stream_item(item5).await;
        Ok(())
    })
}

fn stream_abort_control_removes_state() -> Result<(), &'static str> {
    crate::block_on(async {
        let (registry, mut work_rx) = common::make_provider("abort").await;
        let router = InvocationRouter::with_providers(registry);

        saikuro_exec::spawn(async move { while work_rx.recv().await.is_some() {} });

        let env = Envelope::stream_open("abort.feed", crate::vec![]).map_err(|_| "create")?;
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
        check_test!(result.is_ok(), "abort frame must route");

        // Subsequent routing should fail: state has been removed.
        let extra = ResponseEnvelope::stream_item(id, 1, Value::Null);
        check_test!(
            router.route_stream_item(extra).await.is_err(),
            "routing after abort must fail"
        );
        Ok(())
    })
}

fn concurrent_stream_delivery_preserves_order_and_terminal_closure() -> Result<(), &'static str> {
    crate::block_on(async {
        let id = InvocationId::new().map_err(|_| "entropy")?;
        let (tx, mut rx) = saikuro_exec::mpsc::channel(saikuro_exec::ChannelCapacity::MIN);
        tx.send(ResponseEnvelope::ok_empty(id))
            .await
            .map_err(|_| "receiver remains open")?;
        let state = StreamState::new(tx);

        let first = state.deliver(ResponseEnvelope::stream_item(id, 0, Value::Int(0)));
        pin_mut!(first);
        check_test!(
            matches!(poll!(first.as_mut()), Poll::Pending),
            "first delivery must be pending on a full queue"
        );

        let terminal = state.deliver(ResponseEnvelope::stream_end(id, 1));
        pin_mut!(terminal);
        check_test!(
            matches!(poll!(terminal.as_mut()), Poll::Pending),
            "terminal delivery must be pending on a full queue"
        );

        check_test!(rx.recv().await.is_some(), "drain queued item");
        assert_eq!(first.await, DeliveryOutcome::Delivered);
        check_test!(rx.recv().await.and_then(|r| r.seq) == Some(0), "seq 0 item");
        assert_eq!(terminal.await, DeliveryOutcome::Terminal);
        let end = rx.recv().await.ok_or("terminal frame is delivered")?;
        assert_eq!(end.seq, Some(1));
        assert_eq!(end.stream_control, Some(StreamControl::End));

        assert_eq!(
            state
                .deliver(ResponseEnvelope::stream_item(id, 2, Value::Int(2)))
                .await,
            DeliveryOutcome::Closed
        );
        let recv_fut = rx.recv();
        pin_mut!(recv_fut);
        check_test!(
            matches!(poll!(recv_fut.as_mut()), Poll::Pending),
            "post-terminal frame was not delivered"
        );
        Ok(())
    })
}