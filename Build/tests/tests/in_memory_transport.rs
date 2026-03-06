//! In-memory transport tests

use bytes::Bytes;
use saikuro_transport::{
    memory::MemoryTransport,
    traits::{Transport, TransportReceiver, TransportSender},
};

//  Tests

#[tokio::test]
async fn send_and_receive_single_frame() {
    let (a, b) = MemoryTransport::connected_pair();
    let (mut sender, _) = a.split();
    let (_, mut receiver) = b.split();

    let payload = Bytes::from_static(b"hello saikuro");
    sender.send(payload.clone()).await.expect("send");

    let received = receiver.recv().await.expect("recv ok").expect("some frame");
    assert_eq!(received, payload);
}

#[tokio::test]
async fn multiple_frames_arrive_in_order() {
    let (a, b) = MemoryTransport::connected_pair();
    let (mut sender, _) = a.split();
    let (_, mut receiver) = b.split();

    let frames: Vec<Bytes> = (0u8..10).map(|i| Bytes::from(vec![i; 16])).collect();

    for frame in &frames {
        sender.send(frame.clone()).await.expect("send");
    }

    for expected in &frames {
        let got = receiver.recv().await.expect("recv").expect("frame");
        assert_eq!(&got, expected);
    }
}

#[tokio::test]
async fn recv_returns_none_when_all_senders_dropped() {
    let (a, b) = MemoryTransport::connected_pair();
    let (sender, _) = a.split();
    let (_, mut receiver) = b.split();

    // Drop the sender:  the channel closes.
    drop(sender);

    let result = receiver.recv().await.expect("recv should not error");
    assert!(result.is_none(), "expected None after sender dropped");
}

#[tokio::test]
async fn send_returns_error_when_receiver_dropped() {
    let (a, b) = MemoryTransport::connected_pair();
    let (mut sender, _) = a.split();
    let (_, receiver) = b.split();

    // Drop the receiver.
    drop(receiver);

    let result = sender.send(Bytes::from_static(b"test")).await;
    assert!(result.is_err(), "send should fail with receiver dropped");
}

#[tokio::test]
async fn bidirectional_exchange() {
    let (a, b) = MemoryTransport::connected_pair();
    let (mut a_tx, mut a_rx) = a.split();
    let (mut b_tx, mut b_rx) = b.split();

    // A sends to B.
    a_tx.send(Bytes::from_static(b"ping")).await.unwrap();
    let ping = b_rx.recv().await.unwrap().unwrap();
    assert_eq!(ping, Bytes::from_static(b"ping"));

    // B replies to A.
    b_tx.send(Bytes::from_static(b"pong")).await.unwrap();
    let pong = a_rx.recv().await.unwrap().unwrap();
    assert_eq!(pong, Bytes::from_static(b"pong"));
}

#[tokio::test]
async fn empty_frame_is_transmitted_faithfully() {
    let (a, b) = MemoryTransport::connected_pair();
    let (mut sender, _) = a.split();
    let (_, mut receiver) = b.split();

    sender.send(Bytes::new()).await.unwrap();
    let got = receiver.recv().await.unwrap().unwrap();
    assert!(got.is_empty());
}

#[tokio::test]
async fn large_frame_is_transmitted_faithfully() {
    let (a, b) = MemoryTransport::connected_pair();
    let (mut sender, _) = a.split();
    let (_, mut receiver) = b.split();

    // 1 MiB frame.
    let big = Bytes::from(vec![0xABu8; 1024 * 1024]);
    sender.send(big.clone()).await.unwrap();
    let got = receiver.recv().await.unwrap().unwrap();
    assert_eq!(got, big);
}

#[tokio::test]
async fn close_sender_signals_eof_to_receiver() {
    let (a, b) = MemoryTransport::connected_pair();
    let (mut sender, _) = a.split();
    let (_, mut receiver) = b.split();

    sender.send(Bytes::from_static(b"last")).await.unwrap();
    sender.close().await.unwrap();
    drop(sender);

    let frame = receiver.recv().await.unwrap().unwrap();
    assert_eq!(frame, Bytes::from_static(b"last"));

    // After the sender is closed, recv should return None.
    let eof = receiver.recv().await.unwrap();
    assert!(eof.is_none());
}

#[tokio::test]
async fn named_pair_label_does_not_affect_behaviour() {
    let (a, b) = MemoryTransport::pair("producer", "consumer");
    let (mut sender, _) = a.split();
    let (_, mut receiver) = b.split();

    let data = Bytes::from(b"labeled".to_vec());
    sender.send(data.clone()).await.unwrap();
    assert_eq!(receiver.recv().await.unwrap().unwrap(), data);
}

#[tokio::test]
async fn concurrent_send_and_receive() {
    use std::sync::Arc;
    use tokio::sync::Barrier;

    let (a, b) = MemoryTransport::connected_pair();
    let (mut sender, _) = a.split();
    let (_, mut receiver) = b.split();

    const N: usize = 100;
    let barrier = Arc::new(Barrier::new(2));

    let b2 = barrier.clone();
    let sender_task = tokio::spawn(async move {
        b2.wait().await;
        for i in 0..N {
            sender.send(Bytes::from(vec![i as u8])).await.unwrap();
        }
    });

    let recv_task = tokio::spawn(async move {
        barrier.wait().await;
        let mut received = Vec::with_capacity(N);
        for _ in 0..N {
            let frame = receiver.recv().await.unwrap().unwrap();
            received.push(frame[0]);
        }
        received
    });

    sender_task.await.unwrap();
    let received = recv_task.await.unwrap();

    // All N items must arrive; in-order guaranteed.
    assert_eq!(received, (0..N).map(|i| i as u8).collect::<Vec<_>>());
}
