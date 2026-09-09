//! Channel dispatch integration tests.

use crate::check_test;
use crate::common;
use crate::shared_test;
use crate::TestSuite;
use core::task::Poll;
use futures::{pin_mut, poll};
use saikuro_core::{
    envelope::{Envelope, StreamControl},
    invocation::InvocationId,
    ResponseEnvelope,
};
use saikuro_event::{ErrorCode, Value};
use saikuro_exec::mpsc;
use saikuro_router::{
    provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem},
    router::InvocationRouter,
    stream_state::{ChannelState, DeliveryOutcome},
};

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "router::channel_open_returns_ok_empty",
        channel_open_returns_ok_empty,
    );
    shared_test!(
        suite,
        "router::channel_open_to_unknown_namespace_returns_no_provider",
        channel_open_to_unknown_namespace_returns_no_provider,
    );
    shared_test!(
        suite,
        "router::route_channel_inbound_delivers_to_state",
        route_channel_inbound_delivers_to_state,
    );
    shared_test!(
        suite,
        "router::route_channel_outbound_delivers_to_state",
        route_channel_outbound_delivers_to_state,
    );
    shared_test!(
        suite,
        "router::route_channel_inbound_end_removes_state",
        route_channel_inbound_end_removes_state,
    );
    shared_test!(
        suite,
        "router::route_channel_outbound_end_removes_state",
        route_channel_outbound_end_removes_state,
    );
    shared_test!(
        suite,
        "router::route_channel_abort_removes_state",
        route_channel_abort_removes_state,
    );
    shared_test!(
        suite,
        "router::route_channel_inbound_to_unknown_channel_fails",
        route_channel_inbound_to_unknown_channel_fails,
    );
    shared_test!(
        suite,
        "router::route_channel_outbound_to_unknown_channel_fails",
        route_channel_outbound_to_unknown_channel_fails,
    );
    shared_test!(
        suite,
        "router::multiple_channels_are_independent",
        multiple_channels_are_independent,
    );
    shared_test!(
        suite,
        "router::channel_open_to_dropped_provider_returns_unavailable",
        channel_open_to_dropped_provider_returns_unavailable,
    );
    shared_test!(
        suite,
        "router::channel_pause_resume_round_trips",
        channel_pause_resume_round_trips,
    );
    shared_test!(
        suite,
        "router::concurrent_channel_delivery_preserves_order_and_terminal_closure",
        concurrent_channel_delivery_preserves_order_and_terminal_closure,
    );
}

fn channel_item(id: InvocationId, seq: u64, value: Value) -> ResponseEnvelope {
    ResponseEnvelope {
        id,
        ok: true,
        result: Some(value),
        error: None,
        seq: Some(seq),
        stream_control: None,
    }
}

fn channel_end(id: InvocationId, seq: u64) -> ResponseEnvelope {
    ResponseEnvelope {
        id,
        ok: true,
        result: None,
        error: None,
        seq: Some(seq),
        stream_control: Some(StreamControl::End),
    }
}

fn channel_abort(id: InvocationId, seq: u64) -> ResponseEnvelope {
    ResponseEnvelope {
        id,
        ok: false,
        result: None,
        error: None,
        seq: Some(seq),
        stream_control: Some(StreamControl::Abort),
    }
}

fn channel_open_returns_ok_empty() -> Result<(), &'static str> {
    crate::block_on(async {
        let (registry, mut work_rx) = common::make_provider("chat").await;
        saikuro_exec::spawn(async move { while (work_rx.recv().await).is_some() {} });

        let router = InvocationRouter::with_providers(registry);
        let env = Envelope::channel_open("chat.open", crate::vec![Value::String("room1".into())])
            .map_err(|_| "create")?;
        let resp = router.dispatch(env).await;

        check_test!(resp.ok, "channel open should return ok");
        check_test!(resp.result.is_none(), "channel open returns empty result");
        check_test!(resp.stream_control.is_none(), "no stream control on open");
        Ok(())
    })
}

fn channel_open_to_unknown_namespace_returns_no_provider() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = ProviderRegistry::new();
        let router = InvocationRouter::with_providers(registry);

        let env = Envelope::channel_open("ghost.open", crate::vec![]).map_err(|_| "create")?;
        let resp = router.dispatch(env).await;

        check_test!(!resp.ok, "unknown namespace must fail");
        assert_eq!(
            resp.error.as_ref().map(|e| e.code.clone()),
            Some(ErrorCode::NoProvider)
        );
        Ok(())
    })
}

fn route_channel_inbound_delivers_to_state() -> Result<(), &'static str> {
    crate::block_on(async {
        let (registry, mut work_rx) = common::make_provider("pipe").await;
        let router = InvocationRouter::with_providers(registry);
        let open_env =
            Envelope::channel_open("pipe.connect", crate::vec![]).map_err(|_| "create")?;
        let channel_id = open_env.id;

        saikuro_exec::spawn(async move { while (work_rx.recv().await).is_some() {} });

        let open_resp = router.dispatch(open_env).await;
        check_test!(open_resp.ok, "channel must open");

        // Take the inbound receiver so the channel stays drainable.
        let mut inbound_rx = router
            .streams()
            .take_channel_inbound_receiver(&channel_id)
            .await
            .ok_or("inbound receiver must exist after channel open")?;

        // Route an inbound item from the client.
        let item = channel_item(channel_id, 0, Value::String("hello".into()));
        let result = router.route_channel_inbound(item).await;
        check_test!(
            result.is_ok(),
            "routing a valid inbound item should succeed"
        );

        // Confirm the item arrived on the inbound queue.
        let received = inbound_rx
            .recv()
            .await
            .ok_or("inbound item should be buffered")?;
        assert_eq!(received.result, Some(Value::String("hello".into())));
        Ok(())
    })
}

fn route_channel_outbound_delivers_to_state() -> Result<(), &'static str> {
    crate::block_on(async {
        let (registry, mut work_rx) = common::make_provider("pipe2").await;
        let router = InvocationRouter::with_providers(registry);
        let open_env =
            Envelope::channel_open("pipe2.connect", crate::vec![]).map_err(|_| "create")?;
        let channel_id = open_env.id;

        saikuro_exec::spawn(async move { while (work_rx.recv().await).is_some() {} });

        router.dispatch(open_env).await;

        // Take the outbound receiver so the channel stays drainable.
        let mut outbound_rx = router
            .streams()
            .take_channel_outbound_receiver(&channel_id)
            .await
            .ok_or("outbound receiver must exist after channel open")?;

        // Provider pushes a message to the client.
        let item = channel_item(channel_id, 0, Value::Int(42));
        let result = router.route_channel_outbound(item).await;
        check_test!(result.is_ok(), "outbound item should route");

        let received = outbound_rx
            .recv()
            .await
            .ok_or("outbound item should be buffered")?;
        assert_eq!(received.result, Some(Value::Int(42)));
        Ok(())
    })
}

fn route_channel_inbound_end_removes_state() -> Result<(), &'static str> {
    crate::block_on(async {
        let (registry, mut work_rx) = common::make_provider("fin_chan").await;
        let router = InvocationRouter::with_providers(registry);
        let open_env =
            Envelope::channel_open("fin_chan.open", crate::vec![]).map_err(|_| "create")?;
        let channel_id = open_env.id;

        saikuro_exec::spawn(async move { while (work_rx.recv().await).is_some() {} });
        router.dispatch(open_env).await;

        // Consume the receiver so sends don't fail.
        let _rx = router
            .streams()
            .take_channel_inbound_receiver(&channel_id)
            .await;

        // Send end-of-channel from the client side.
        let end = channel_end(channel_id, 0);
        let result = router.route_channel_inbound(end).await;
        check_test!(result.is_ok(), "end must route");

        // After end the channel state is removed: routing another item fails.
        let extra = channel_item(channel_id, 1, Value::Null);
        check_test!(
            router.route_channel_inbound(extra).await.is_err(),
            "routing to closed channel should fail"
        );
        Ok(())
    })
}

fn route_channel_outbound_end_removes_state() -> Result<(), &'static str> {
    crate::block_on(async {
        let (registry, mut work_rx) = common::make_provider("fin_out").await;
        let router = InvocationRouter::with_providers(registry);
        let open_env =
            Envelope::channel_open("fin_out.open", crate::vec![]).map_err(|_| "create")?;
        let channel_id = open_env.id;

        saikuro_exec::spawn(async move { while (work_rx.recv().await).is_some() {} });
        router.dispatch(open_env).await;

        let _rx = router
            .streams()
            .take_channel_outbound_receiver(&channel_id)
            .await;

        let end = channel_end(channel_id, 0);
        let result = router.route_channel_outbound(end).await;
        check_test!(result.is_ok(), "outbound end must route");

        let extra = channel_item(channel_id, 1, Value::Null);
        check_test!(
            router.route_channel_outbound(extra).await.is_err(),
            "routing to closed channel should fail after outbound end"
        );
        Ok(())
    })
}

fn route_channel_abort_removes_state() -> Result<(), &'static str> {
    crate::block_on(async {
        let (registry, mut work_rx) = common::make_provider("abort_chan").await;
        let router = InvocationRouter::with_providers(registry);
        let open_env =
            Envelope::channel_open("abort_chan.open", crate::vec![]).map_err(|_| "create")?;
        let channel_id = open_env.id;

        saikuro_exec::spawn(async move { while (work_rx.recv().await).is_some() {} });
        router.dispatch(open_env).await;

        let _rx = router
            .streams()
            .take_channel_inbound_receiver(&channel_id)
            .await;

        let abort = channel_abort(channel_id, 0);
        let result = router.route_channel_inbound(abort).await;
        check_test!(result.is_ok(), "abort must route");

        // State removed after abort.
        let extra = channel_item(channel_id, 1, Value::Null);
        check_test!(
            router.route_channel_inbound(extra).await.is_err(),
            "routing to aborted channel should fail"
        );
        Ok(())
    })
}

fn route_channel_inbound_to_unknown_channel_fails() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = ProviderRegistry::new();
        let router = InvocationRouter::with_providers(registry);

        let phantom_id = InvocationId::new().map_err(|_| "entropy")?;
        let item = channel_item(phantom_id, 0, Value::Null);
        check_test!(
            router.route_channel_inbound(item).await.is_err(),
            "routing to non-existent channel should fail"
        );
        Ok(())
    })
}

fn route_channel_outbound_to_unknown_channel_fails() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = ProviderRegistry::new();
        let router = InvocationRouter::with_providers(registry);

        let phantom_id = InvocationId::new().map_err(|_| "entropy")?;
        let item = channel_item(phantom_id, 0, Value::Null);
        check_test!(
            router.route_channel_outbound(item).await.is_err(),
            "routing outbound to non-existent channel should fail"
        );
        Ok(())
    })
}

fn multiple_channels_are_independent() -> Result<(), &'static str> {
    crate::block_on(async {
        let (registry, mut work_rx) = common::make_provider("multi_chan").await;
        let router = InvocationRouter::with_providers(registry);

        saikuro_exec::spawn(async move { while (work_rx.recv().await).is_some() {} });

        let env1 = Envelope::channel_open("multi_chan.ch1", crate::vec![]).map_err(|_| "create")?;
        let env2 = Envelope::channel_open("multi_chan.ch2", crate::vec![]).map_err(|_| "create")?;
        let id1 = env1.id;
        let id2 = env2.id;

        router.dispatch(env1).await;
        router.dispatch(env2).await;

        let _rx1_in = router.streams().take_channel_inbound_receiver(&id1).await;
        let _rx2_in = router.streams().take_channel_inbound_receiver(&id2).await;

        // Route items to channel 1.
        let item1 = channel_item(id1, 0, Value::Int(1));
        check_test!(
            router.route_channel_inbound(item1).await.is_ok(),
            "ch1 item"
        );

        // Route items to channel 2.
        let item2 = channel_item(id2, 0, Value::Int(2));
        check_test!(
            router.route_channel_inbound(item2).await.is_ok(),
            "ch2 item"
        );

        // Close channel 1; channel 2 must still be alive.
        let end1 = channel_end(id1, 1);
        router.route_channel_inbound(end1).await.ok();

        let item2b = channel_item(id2, 1, Value::Int(99));
        check_test!(
            router.route_channel_inbound(item2b).await.is_ok(),
            "channel 2 should still accept items after channel 1 closes"
        );
        Ok(())
    })
}

fn channel_open_to_dropped_provider_returns_unavailable() -> Result<(), &'static str> {
    crate::block_on(async {
        let (work_tx, work_rx) =
            mpsc::channel::<ProviderWorkItem>(saikuro_exec::ChannelCapacity::MIN);
        let handle = ProviderHandle::new(
            crate::String::from("dropped-provider"),
            crate::vec![crate::String::from("dropped")],
            work_tx,
        );
        let registry = ProviderRegistry::new();
        registry.register(handle).await;

        // Drop the receiver: provider is now unavailable.
        drop(work_rx);

        let router = InvocationRouter::with_providers(registry);
        let env = Envelope::channel_open("dropped.open", crate::vec![]).map_err(|_| "create")?;
        let resp = router.dispatch(env).await;

        check_test!(!resp.ok, "dropped provider must fail");
        let err = resp.error.as_ref().map(|e| e.code.clone()).unwrap();
        assert!(
            err == ErrorCode::ProviderUnavailable || err == ErrorCode::NoProvider,
            "expected provider unavailable/no-provider, got {:?}",
            err
        );
        Ok(())
    })
}

fn channel_pause_resume_round_trips() -> Result<(), &'static str> {
    crate::block_on(async {
        let (registry, mut work_rx) = common::make_provider("bpressure").await;
        let router = InvocationRouter::with_providers(registry);
        let open_env =
            Envelope::channel_open("bpressure.stream", crate::vec![]).map_err(|_| "create")?;
        let channel_id = open_env.id;

        saikuro_exec::spawn(async move { while (work_rx.recv().await).is_some() {} });
        router.dispatch(open_env).await;

        let mut outbound_rx = router
            .streams()
            .take_channel_outbound_receiver(&channel_id)
            .await
            .ok_or("outbound receiver must exist")?;

        // Provider sends a Pause control frame to signal backpressure.
        let pause = ResponseEnvelope {
            id: channel_id,
            ok: true,
            result: None,
            error: None,
            seq: Some(0),
            stream_control: Some(StreamControl::Pause),
        };
        check_test!(
            router.route_channel_outbound(pause).await.is_ok(),
            "pause must route"
        );

        let received = outbound_rx
            .recv()
            .await
            .ok_or("pause frame should be buffered")?;
        assert_eq!(received.stream_control, Some(StreamControl::Pause));

        // Provider sends a Resume frame: channel must still be open (Pause is
        // not terminal).
        let resume = ResponseEnvelope {
            id: channel_id,
            ok: true,
            result: None,
            error: None,
            seq: Some(1),
            stream_control: Some(StreamControl::Resume),
        };
        check_test!(
            router.route_channel_outbound(resume).await.is_ok(),
            "channel should still accept frames after Pause"
        );

        let received2 = outbound_rx
            .recv()
            .await
            .ok_or("resume frame should be buffered")?;
        assert_eq!(received2.stream_control, Some(StreamControl::Resume));
        Ok(())
    })
}

fn concurrent_channel_delivery_preserves_order_and_terminal_closure() -> Result<(), &'static str> {
    crate::block_on(async {
        let id = InvocationId::new().map_err(|_| "entropy")?;
        let (inbound_tx, mut inbound_rx) = mpsc::channel(saikuro_exec::ChannelCapacity::MIN);
        let (outbound_tx, mut outbound_rx) = mpsc::channel(saikuro_exec::ChannelCapacity::MIN);
        inbound_tx
            .send(ResponseEnvelope::ok_empty(id))
            .await
            .map_err(|_| "receiver remains open")?;
        let state = ChannelState::new(inbound_tx, outbound_tx);

        let first = state.deliver(channel_item(id, 0, Value::Int(0)), true);
        pin_mut!(first);
        check_test!(
            matches!(poll!(first.as_mut()), Poll::Pending),
            "first delivery must be pending on a full queue"
        );

        let terminal = state.deliver(channel_end(id, 1), true);
        pin_mut!(terminal);
        check_test!(
            matches!(poll!(terminal.as_mut()), Poll::Pending),
            "terminal delivery must be pending on a full queue"
        );

        check_test!(inbound_rx.recv().await.is_some(), "drain queued item");
        assert_eq!(first.await, DeliveryOutcome::Delivered);
        assert_eq!(
            inbound_rx.recv().await.and_then(|response| response.seq),
            Some(0)
        );
        assert_eq!(terminal.await, DeliveryOutcome::Terminal);
        assert_eq!(
            inbound_rx.recv().await.and_then(|response| response.seq),
            Some(1)
        );

        assert_eq!(
            state
                .deliver(channel_item(id, 0, Value::Int(9)), false)
                .await,
            DeliveryOutcome::Closed
        );
        let recv_fut = outbound_rx.recv();
        pin_mut!(recv_fut);
        check_test!(
            matches!(poll!(recv_fut.as_mut()), Poll::Pending),
            "post-terminal frame was not delivered"
        );
        Ok(())
    })
}
