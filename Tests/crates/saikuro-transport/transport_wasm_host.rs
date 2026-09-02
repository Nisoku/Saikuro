//! WasmHostTransport tests (wasm32 only).
//!
//! These tests create a pair of `BroadcastChannel`s with the same name,
//! wrap each in a `WasmHostTransport`, and validate the full `Transport`
//! trait contract.  For the connector/listener handshake, two channels
//! with a well-known base name are used,  one as listener, one as
//! connector, exercising the full rendezvous protocol.
//!
//! Because `BroadcastChannel` requires actual browser APIs, these tests
//! only compile on `wasm32-unknown-unknown` and must be run in a WASM
//! environment (e.g. `wasm-bindgen-test-runner` or a headless browser).

#![cfg(target_arch = "wasm32")]

use bytes::Bytes;
use saikuro_transport::{
    traits::{
        Transport, TransportConnector, TransportListener, TransportReceiver, TransportSender,
    },
    wasm_host::{WasmHostConnector, WasmHostListener, WasmHostTransport},
};
use wasm_bindgen_test::*;
use web_sys::BroadcastChannel;

// helpers

/// Create two WasmHostTransports that share a `BroadcastChannel` by name.
fn make_transport_pair(
    name: &str,
    label_a: &str,
    label_b: &str,
) -> (WasmHostTransport, WasmHostTransport) {
    let a = WasmHostTransport::new(BroadcastChannel::new(name).unwrap(), label_a);
    let b = WasmHostTransport::new(BroadcastChannel::new(name).unwrap(), label_b);
    (a, b)
}

fn make_raw_pair() -> (WasmHostTransport, WasmHostTransport) {
    make_transport_pair("test-wasm-raw", "raw-a", "raw-b")
}

// WasmHostTransport basics (direct BroadcastChannel)
#[wasm_bindgen_test]
async fn send_receive_single_frame() {
    let (a, b) = make_raw_pair();
    let (mut sender, _) = a.split();
    let (_, mut receiver) = b.split();

    let payload = Bytes::from_static(b"hello wasm");
    sender.send(payload.clone()).await.expect("send");
    let received = receiver.recv().await.expect("recv ok").expect("frame");
    assert_eq!(received, payload);
}

#[wasm_bindgen_test]
async fn multiple_frames_in_order() {
    let (a, b) = make_raw_pair();
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
    let (a, b) = make_raw_pair();
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
    let (a, b) = make_raw_pair();
    let (mut sender, _) = a.split();
    let (_, mut receiver) = b.split();

    sender.send(Bytes::new()).await.unwrap();
    let got = receiver.recv().await.unwrap().unwrap();
    assert!(got.is_empty());
}

#[wasm_bindgen_test]
async fn large_frame() {
    let (a, b) = make_raw_pair();
    let (mut sender, _) = a.split();
    let (_, mut receiver) = b.split();

    let big = Bytes::from(vec![0xCDu8; 256 * 1024]); // 256 KiB
    sender.send(big.clone()).await.unwrap();
    let got = receiver.recv().await.unwrap().unwrap();
    assert_eq!(got, big);
}

#[wasm_bindgen_test]
async fn close_sender_signals_eof_to_receiver() {
    let (a, b) = make_raw_pair();
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
async fn concurrent_send_and_receive() {
    let (a, b) = make_raw_pair();
    let (mut sender, _) = a.split();
    let (_, mut receiver) = b.split();

    const N: usize = 50;

    let sender_task = saikuro_exec::spawn(async move {
        for i in 0..N {
            sender.send(Bytes::from(vec![i as u8])).await.unwrap();
            saikuro_exec::yield_now().await;
        }
    });

    let recv_task = saikuro_exec::spawn(async move {
        let mut received = Vec::with_capacity(N);
        while received.len() < N {
            if let Some(frame) = receiver.recv().await.unwrap() {
                received.push(frame[0]);
            }
        }
        received
    });

    sender_task.await.unwrap();
    let received = recv_task.await.unwrap();
    assert_eq!(received.len(), N);
    let mut sorted = received.clone();
    sorted.sort();
    assert_eq!(sorted, (0..N).map(|i| i as u8).collect::<Vec<_>>());
}

#[wasm_bindgen_test]
async fn recv_returns_none_after_transport_dropped() {
    let (a, b) = make_raw_pair();
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
async fn multiple_independent_transports() {
    let (a1, b1) = make_transport_pair("multi-1", "pair1-a", "pair1-b");
    let (a2, b2) = make_transport_pair("multi-2", "pair2-a", "pair2-b");

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
    let (a, _b) = make_raw_pair();
    assert_eq!(a.description(), "wasm-host");
}

// Connector / Listener integration

#[wasm_bindgen_test]
async fn connector_listener_round_trip() {
    let base = "cl-rt-test";
    let mut listener = WasmHostListener::new(base).unwrap();
    let connector = WasmHostConnector::new(base);

    // Spawn accept in background, moving the listener in.
    let (tx, rx) = saikuro_exec::oneshot::channel();
    wasm_bindgen_futures::spawn_local(async move {
        let result = listener.accept().await;
        let _ = tx.send(result);
    });
    saikuro_exec::yield_now().await;

    let transport_a = connector.connect().await.unwrap();
    let transport_b = rx.await.unwrap().unwrap().unwrap();

    let (mut a_tx, mut a_rx) = transport_a.split();
    let (mut b_tx, mut b_rx) = transport_b.split();

    a_tx.send(Bytes::from_static(b"hello")).await.unwrap();
    let got = b_rx.recv().await.unwrap().unwrap();
    assert_eq!(got, Bytes::from_static(b"hello"));

    b_tx.send(Bytes::from_static(b"world")).await.unwrap();
    let got = a_rx.recv().await.unwrap().unwrap();
    assert_eq!(got, Bytes::from_static(b"world"));
}

#[wasm_bindgen_test]
async fn listener_accepts_queued_connect() {
    let base = "lq-test";
    let mut listener = WasmHostListener::new(base).unwrap();

    // Manually send a connect message on the base channel.
    let base_ch = BroadcastChannel::new(base).unwrap();
    let msg = make_connect_msg("queued-id");
    base_ch.post_message(&msg).unwrap();

    // Give the onmessage handler a moment to fire.
    saikuro_exec::yield_now().await;

    let transport = listener.accept().await.unwrap().unwrap();
    assert_eq!(transport.description(), "wasm-host");
}

fn make_connect_msg(conn_id: &str) -> wasm_bindgen::JsValue {
    let obj = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&obj, &"type".into(), &"connect".into());
    let _ = js_sys::Reflect::set(&obj, &"id".into(), &conn_id.into());
    wasm_bindgen::JsValue::from(obj)
}
