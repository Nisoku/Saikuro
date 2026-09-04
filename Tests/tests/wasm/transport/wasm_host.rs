//! WasmHostTransport tests

use bytes::Bytes;
use core::cell::Cell;
use core::time::Duration;
use js_sys::{Object, Reflect};
use saikuro_transport::wasm::{BroadcastChannelPipe, WasmHost};
use saikuro_transport::{
    LocalTransport, LocalTransportConnector, LocalTransportListener, LocalTransportReceiver,
    LocalTransportSender, WasmHostConnector, WasmHostListener,
};
use std::rc::Rc;
use wasm_bindgen_test::*;
use web_sys::BroadcastChannel;

/// Open a `WasmHostTransport` pair over the `BroadcastChannel` rendezvous.
async fn make_transport_pair(channel: &str) -> (WasmHost, WasmHost) {
    let mut listener = WasmHostListener::<BroadcastChannelPipe>::new(channel);
    let connector = WasmHostConnector::<BroadcastChannelPipe>::new(channel);

    let (tx, rx) = saikuro_exec::oneshot::channel();
    wasm_bindgen_futures::spawn_local(async move {
        let _ = tx.send(listener.accept().await);
    });
    saikuro_exec::yield_now().await;

    let transport_a = connector.connect().await.expect("connect");
    let transport_b = rx
        .await
        .expect("accept")
        .expect("transport")
        .expect("transport option");
    (transport_a, transport_b)
}

// WasmHostTransport basics (frame shipping over BroadcastChannel)
#[wasm_bindgen_test]
async fn send_receive_single_frame() {
    let (a, b) = make_transport_pair("wasm-raw-single").await;
    let (mut sender, _) = a.split();
    let (_, mut receiver) = b.split();

    let payload = Bytes::from_static(b"hello wasm");
    sender.send(payload.clone()).await.expect("send");
    let received = receiver.recv().await.expect("recv ok").expect("frame");
    assert_eq!(received, payload);
}

#[wasm_bindgen_test]
async fn multiple_frames_in_order() {
    let (a, b) = make_transport_pair("wasm-raw-multi").await;
    let (mut sender, _) = a.split();
    let (_, mut receiver) = b.split();

    let frames: Vec<Bytes> = (0u8..20).map(|i| Bytes::from(vec![i; 32])).collect();
    for frame in &frames {
        sender.send(frame.clone()).await.expect("send");
    }
    saikuro_exec::yield_now().await;

    for expected in &frames {
        let got = receiver.recv().await.expect("recv").expect("frame");
        assert_eq!(&got, expected);
    }
}

#[wasm_bindgen_test]
async fn bidirectional_exchange() {
    let (a, b) = make_transport_pair("wasm-raw-bidi").await;
    let (mut a_tx, mut a_rx) = a.split();
    let (mut b_tx, mut b_rx) = b.split();

    a_tx.send(Bytes::from_static(b"ping")).await.unwrap();
    let ping = b_rx.recv().await.unwrap().unwrap();
    assert_eq!(ping, Bytes::from_static(b"ping"));

    b_tx.send(Bytes::from_static(b"pong")).await.unwrap();
    let pong = a_rx.recv().await.unwrap().unwrap();
    assert_eq!(pong, Bytes::from_static(b"pong"));
}

#[wasm_bindgen_test]
async fn empty_frame() {
    let (a, b) = make_transport_pair("wasm-raw-empty").await;
    let (mut sender, _) = a.split();
    let (_, mut receiver) = b.split();

    sender.send(Bytes::new()).await.unwrap();
    let got = receiver.recv().await.unwrap().unwrap();
    assert!(got.is_empty());
}

#[wasm_bindgen_test]
async fn large_frame() {
    let (a, b) = make_transport_pair("wasm-raw-large").await;
    let (mut sender, _) = a.split();
    let (_, mut receiver) = b.split();

    let big = Bytes::from(vec![0xCDu8; 256 * 1024]); // 256 KiB
    sender.send(big.clone()).await.unwrap();
    let got = receiver.recv().await.unwrap().unwrap();
    assert_eq!(got, big);
}

#[wasm_bindgen_test]
async fn close_sender_signals_eof_to_receiver() {
    let (a, b) = make_transport_pair("wasm-raw-eof").await;
    let (mut sender, _) = a.split();
    let (_, mut receiver) = b.split();

    sender.send(Bytes::from_static(b"last")).await.unwrap();
    sender.close().await.unwrap();
    drop(sender);

    let frame = receiver.recv().await.unwrap().unwrap();
    assert_eq!(frame, Bytes::from_static(b"last"));

    for _ in 0..5 {
        if receiver.recv().await.unwrap().is_none() {
            return;
        }
        saikuro_exec::yield_now().await;
    }
    panic!("receiver did not see EOF after sender close + drop");
}

#[wasm_bindgen_test]
async fn recv_returns_none_after_transport_dropped() {
    let (a, b) = make_transport_pair("wasm-raw-drop").await;
    let (_, mut receiver) = b.split();
    drop(a);
    for _ in 0..10 {
        if receiver.recv().await.unwrap().is_none() {
            return;
        }
        saikuro_exec::yield_now().await;
    }
    panic!("receiver did not yield None after transport was dropped");
}

#[wasm_bindgen_test]
async fn concurrent_send_and_receive() {
    let (a, b) = make_transport_pair("wasm-raw-concurrent").await;
    let (mut sender, _) = a.split();
    let (_, mut receiver) = b.split();

    const N: usize = 50;

    let (send_tx, send_rx) = saikuro_exec::oneshot::channel();
    let (recv_tx, recv_rx) = saikuro_exec::oneshot::channel();

    wasm_bindgen_futures::spawn_local(async move {
        for i in 0..N {
            sender.send(Bytes::from(vec![i as u8])).await.unwrap();
            saikuro_exec::yield_now().await;
        }
        let _ = send_tx.send(());
    });
    wasm_bindgen_futures::spawn_local(async move {
        let mut received = Vec::with_capacity(N);
        while received.len() < N {
            if let Some(frame) = receiver.recv().await.unwrap() {
                received.push(frame[0]);
            }
        }
        let _ = recv_tx.send(received);
    });

    send_rx.await.expect("sender task finished");
    let received = recv_rx.await.expect("receiver task finished");
    assert_eq!(received.len(), N);
    let mut sorted = received.clone();
    sorted.sort();
    assert_eq!(sorted, (0..N).map(|i| i as u8).collect::<Vec<_>>());
}

#[wasm_bindgen_test]
async fn multiple_independent_transports() {
    let (a1, b1) = make_transport_pair("wasm-multi-1").await;
    let (a2, b2) = make_transport_pair("wasm-multi-2").await;

    let (mut a1_tx, _) = a1.split();
    let (_, mut b1_rx) = b1.split();
    let (mut a2_tx, _) = a2.split();
    let (_, mut b2_rx) = b2.split();

    a1_tx.send(Bytes::from_static(b"channel-1")).await.unwrap();
    a2_tx.send(Bytes::from_static(b"channel-2")).await.unwrap();

    let from_1 = b1_rx.recv().await.unwrap().unwrap();
    let from_2 = b2_rx.recv().await.unwrap().unwrap();
    assert_eq!(from_1, Bytes::from_static(b"channel-1"));
    assert_eq!(from_2, Bytes::from_static(b"channel-2"));
}

#[wasm_bindgen_test]
async fn transport_description_returns_wasm_host() {
    let (a, _b) = make_transport_pair("wasm-raw-desc").await;
    assert_eq!(a.description(), "wasm-host");
}

// Connector / Listener rendezvous

#[wasm_bindgen_test]
async fn connector_listener_round_trip() {
    let (a, b) = make_transport_pair("wasm-cl-rt").await;
    let (mut a_tx, mut a_rx) = a.split();
    let (mut b_tx, mut b_rx) = b.split();

    a_tx.send(Bytes::from_static(b"hello")).await.unwrap();
    let got = b_rx.recv().await.unwrap().unwrap();
    assert_eq!(got, Bytes::from_static(b"hello"));

    b_tx.send(Bytes::from_static(b"world")).await.unwrap();
    let got = a_rx.recv().await.unwrap().unwrap();
    assert_eq!(got, Bytes::from_static(b"world"));
}

#[wasm_bindgen_test]
async fn listener_accepts_queued_connect() {
    let channel = "wasm-lq";
    let mut listener = WasmHostListener::<BroadcastChannelPipe>::new(channel);

    let (tx, rx) = saikuro_exec::oneshot::channel();
    wasm_bindgen_futures::spawn_local(async move {
        let _ = tx.send(listener.accept().await);
    });

    // The base channel is a `BroadcastChannel`, which is not a queue (a connect
    // announced before the accept handler is attached is silently dropped). So
    // keep re-announcing (idempotent, keyed by `queued-id`) until the listener
    // is wired up and accepts, then stop.
    let stop = Rc::new(Cell::new(false));
    let stop_flag = stop.clone();
    let base_ch = BroadcastChannel::new(channel).expect("base channel");
    wasm_bindgen_futures::spawn_local(async move {
        while !stop_flag.get() {
            base_ch.post_message(&make_connect_msg("queued-id")).unwrap();
            saikuro_exec::sleep(Duration::from_millis(25)).await;
        }
    });

    let transport = saikuro_exec::timeout(Duration::from_secs(5), rx)
        .await
        .expect("listener did not accept a queued connect within 5s")
        .expect("accept")
        .expect("transport")
        .expect("transport option");
    stop.set(true);
    assert_eq!(transport.description(), "wasm-host");
}

fn make_connect_msg(conn_id: &str) -> wasm_bindgen::JsValue {
    let obj = Object::new();
    let _ = Reflect::set(&obj, &"type".into(), &"connect".into());
    let _ = Reflect::set(&obj, &"id".into(), &conn_id.into());
    wasm_bindgen::JsValue::from(obj)
}