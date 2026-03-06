//! Channel dispatch Tests

use saikuro_core::{
    envelope::{Envelope, StreamControl},
    error::ErrorCode,
    invocation::InvocationId,
    value::Value,
    ResponseEnvelope,
};
use saikuro_router::{
    provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem},
    router::InvocationRouter,
};
use tokio::sync::mpsc;

//  Helpers

fn make_provider(namespace: &str) -> (ProviderRegistry, mpsc::Receiver<ProviderWorkItem>) {
    let (work_tx, work_rx) = mpsc::channel::<ProviderWorkItem>(64);
    let handle = ProviderHandle::new(
        format!("{}-provider", namespace),
        vec![namespace.to_owned()],
        work_tx,
    );
    let registry = ProviderRegistry::new();
    registry.register(handle);
    (registry, work_rx)
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

//  Tests

#[tokio::test]
async fn channel_open_returns_ok_empty() {
    let (registry, mut work_rx) = make_provider("chat");

    tokio::spawn(async move { while let Some(_) = work_rx.recv().await {} });

    let router = InvocationRouter::with_providers(registry);
    let env = Envelope::channel_open("chat.open", vec![Value::String("room1".into())]);
    let resp = router.dispatch(env).await;

    assert!(resp.ok, "channel open should return ok");
    assert!(resp.result.is_none(), "channel open returns empty result");
    assert!(resp.stream_control.is_none());
}

#[tokio::test]
async fn channel_open_to_unknown_namespace_returns_no_provider() {
    let registry = ProviderRegistry::new();
    let router = InvocationRouter::with_providers(registry);

    let env = Envelope::channel_open("ghost.open", vec![]);
    let resp = router.dispatch(env).await;

    assert!(!resp.ok);
    let err = resp.error.unwrap();
    assert_eq!(err.code, ErrorCode::NoProvider);
}

#[tokio::test]
async fn route_channel_inbound_delivers_to_state() {
    let (registry, mut work_rx) = make_provider("pipe");

    let router = InvocationRouter::with_providers(registry);
    let open_env = Envelope::channel_open("pipe.connect", vec![]);
    let channel_id = open_env.id;

    tokio::spawn(async move { while let Some(_) = work_rx.recv().await {} });

    let open_resp = router.dispatch(open_env).await;
    assert!(open_resp.ok);

    // Take the inbound receiver so the channel stays drainable.
    let mut inbound_rx = router
        .streams()
        .take_channel_inbound_receiver(&channel_id)
        .expect("inbound receiver must exist after channel open");

    // Route an inbound item from the client.
    let item = channel_item(channel_id, 0, Value::String("hello".into()));
    let result = router.route_channel_inbound(item).await;
    assert!(
        result.is_ok(),
        "routing a valid inbound channel item should succeed"
    );

    // Confirm the item arrived on the inbound queue.
    let received = inbound_rx
        .try_recv()
        .expect("inbound item should be buffered");
    assert_eq!(received.result, Some(Value::String("hello".into())));
}

#[tokio::test]
async fn route_channel_outbound_delivers_to_state() {
    let (registry, mut work_rx) = make_provider("pipe2");

    let router = InvocationRouter::with_providers(registry);
    let open_env = Envelope::channel_open("pipe2.connect", vec![]);
    let channel_id = open_env.id;

    tokio::spawn(async move { while let Some(_) = work_rx.recv().await {} });

    router.dispatch(open_env).await;

    // Take the outbound receiver so the channel stays drainable.
    let mut outbound_rx = router
        .streams()
        .take_channel_outbound_receiver(&channel_id)
        .expect("outbound receiver must exist after channel open");

    // Provider pushes a message to the client.
    let item = channel_item(channel_id, 0, Value::Int(42));
    let result = router.route_channel_outbound(item).await;
    assert!(
        result.is_ok(),
        "routing a valid outbound channel item should succeed"
    );

    let received = outbound_rx
        .try_recv()
        .expect("outbound item should be buffered");
    assert_eq!(received.result, Some(Value::Int(42)));
}

#[tokio::test]
async fn route_channel_inbound_end_removes_state() {
    let (registry, mut work_rx) = make_provider("fin_chan");

    let router = InvocationRouter::with_providers(registry);
    let open_env = Envelope::channel_open("fin_chan.open", vec![]);
    let channel_id = open_env.id;

    tokio::spawn(async move { while let Some(_) = work_rx.recv().await {} });
    router.dispatch(open_env).await;

    // Consume the receiver so sends don't fail.
    let _rx = router.streams().take_channel_inbound_receiver(&channel_id);

    // Send end-of-channel from the client side.
    let end = channel_end(channel_id, 0);
    let result = router.route_channel_inbound(end).await;
    assert!(result.is_ok());

    // After end the channel state is removed:  routing another item should fail.
    let extra = channel_item(channel_id, 1, Value::Null);
    let err = router.route_channel_inbound(extra).await;
    assert!(err.is_err(), "routing to closed channel should fail");
}

#[tokio::test]
async fn route_channel_outbound_end_removes_state() {
    let (registry, mut work_rx) = make_provider("fin_out");

    let router = InvocationRouter::with_providers(registry);
    let open_env = Envelope::channel_open("fin_out.open", vec![]);
    let channel_id = open_env.id;

    tokio::spawn(async move { while let Some(_) = work_rx.recv().await {} });
    router.dispatch(open_env).await;

    let _rx = router.streams().take_channel_outbound_receiver(&channel_id);

    let end = channel_end(channel_id, 0);
    let result = router.route_channel_outbound(end).await;
    assert!(result.is_ok());

    let extra = channel_item(channel_id, 1, Value::Null);
    let err = router.route_channel_outbound(extra).await;
    assert!(
        err.is_err(),
        "routing to closed channel should fail after outbound end"
    );
}

#[tokio::test]
async fn route_channel_abort_removes_state() {
    let (registry, mut work_rx) = make_provider("abort_chan");

    let router = InvocationRouter::with_providers(registry);
    let open_env = Envelope::channel_open("abort_chan.open", vec![]);
    let channel_id = open_env.id;

    tokio::spawn(async move { while let Some(_) = work_rx.recv().await {} });
    router.dispatch(open_env).await;

    let _rx = router.streams().take_channel_inbound_receiver(&channel_id);

    let abort = channel_abort(channel_id, 0);
    let result = router.route_channel_inbound(abort).await;
    assert!(result.is_ok());

    // State removed after abort.
    let extra = channel_item(channel_id, 1, Value::Null);
    let err = router.route_channel_inbound(extra).await;
    assert!(err.is_err(), "routing to aborted channel should fail");
}

#[tokio::test]
async fn route_channel_inbound_to_unknown_channel_fails() {
    let registry = ProviderRegistry::new();
    let router = InvocationRouter::with_providers(registry);

    let phantom_id = InvocationId::new();
    let item = channel_item(phantom_id, 0, Value::Null);
    let err = router.route_channel_inbound(item).await;
    assert!(err.is_err(), "routing to non-existent channel should fail");
}

#[tokio::test]
async fn route_channel_outbound_to_unknown_channel_fails() {
    let registry = ProviderRegistry::new();
    let router = InvocationRouter::with_providers(registry);

    let phantom_id = InvocationId::new();
    let item = channel_item(phantom_id, 0, Value::Null);
    let err = router.route_channel_outbound(item).await;
    assert!(
        err.is_err(),
        "routing outbound to non-existent channel should fail"
    );
}

#[tokio::test]
async fn multiple_channels_are_independent() {
    let (registry, mut work_rx) = make_provider("multi_chan");
    let router = InvocationRouter::with_providers(registry);

    tokio::spawn(async move { while let Some(_) = work_rx.recv().await {} });

    let env1 = Envelope::channel_open("multi_chan.ch1", vec![]);
    let env2 = Envelope::channel_open("multi_chan.ch2", vec![]);
    let id1 = env1.id;
    let id2 = env2.id;

    router.dispatch(env1).await;
    router.dispatch(env2).await;

    let _rx1_in = router.streams().take_channel_inbound_receiver(&id1);
    let _rx2_in = router.streams().take_channel_inbound_receiver(&id2);

    // Route items to channel 1.
    let item1 = channel_item(id1, 0, Value::Int(1));
    assert!(router.route_channel_inbound(item1).await.is_ok());

    // Route items to channel 2.
    let item2 = channel_item(id2, 0, Value::Int(2));
    assert!(router.route_channel_inbound(item2).await.is_ok());

    // Close channel 1; channel 2 must still be alive.
    let end1 = channel_end(id1, 1);
    router.route_channel_inbound(end1).await.ok();

    let item2b = channel_item(id2, 1, Value::Int(99));
    assert!(
        router.route_channel_inbound(item2b).await.is_ok(),
        "channel 2 should still accept items after channel 1 closes"
    );
}

#[tokio::test]
async fn channel_open_to_dropped_provider_returns_unavailable() {
    let (work_tx, work_rx) = mpsc::channel::<ProviderWorkItem>(1);
    let handle = ProviderHandle::new(
        "dropped-provider".to_owned(),
        vec!["dropped".to_owned()],
        work_tx,
    );
    let registry = ProviderRegistry::new();
    registry.register(handle);

    // Drop the receiver:  provider is now unavailable.
    drop(work_rx);

    let router = InvocationRouter::with_providers(registry);
    let env = Envelope::channel_open("dropped.open", vec![]);
    let resp = router.dispatch(env).await;

    assert!(!resp.ok);
    let err = resp.error.unwrap();
    assert!(
        err.code == ErrorCode::ProviderUnavailable || err.code == ErrorCode::NoProvider,
        "expected provider unavailable/no-provider, got {:?}",
        err.code
    );
}

#[tokio::test]
async fn channel_pause_resume_round_trips() {
    let (registry, mut work_rx) = make_provider("bpressure");

    let router = InvocationRouter::with_providers(registry);
    let open_env = Envelope::channel_open("bpressure.stream", vec![]);
    let channel_id = open_env.id;

    tokio::spawn(async move { while let Some(_) = work_rx.recv().await {} });
    router.dispatch(open_env).await;

    let mut outbound_rx = router
        .streams()
        .take_channel_outbound_receiver(&channel_id)
        .expect("outbound receiver must exist");

    // Provider sends a Pause control frame to signal backpressure.
    let pause = ResponseEnvelope {
        id: channel_id,
        ok: true,
        result: None,
        error: None,
        seq: Some(0),
        stream_control: Some(StreamControl::Pause),
    };
    assert!(router.route_channel_outbound(pause).await.is_ok());

    let received = outbound_rx
        .try_recv()
        .expect("pause frame should be buffered");
    assert_eq!(received.stream_control, Some(StreamControl::Pause));

    // Provider sends a Resume frame:  channel must still be open (Pause is not terminal).
    let resume = ResponseEnvelope {
        id: channel_id,
        ok: true,
        result: None,
        error: None,
        seq: Some(1),
        stream_control: Some(StreamControl::Resume),
    };
    assert!(
        router.route_channel_outbound(resume).await.is_ok(),
        "channel should still accept frames after Pause"
    );

    let received2 = outbound_rx
        .try_recv()
        .expect("resume frame should be buffered");
    assert_eq!(received2.stream_control, Some(StreamControl::Resume));
}
