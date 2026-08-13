//! Transport trait compliance tests.
//!
//! A shared test suite that every Transport implementation must pass.
//! To test a new transport, call [`run_transport_compliance`] with a
//! factory that produces a connected pair.
//!
//! Currently validated backends:
//! - [`MemoryTransport`] (always runs)

use bytes::Bytes;
use saikuro_exec::sync::Barrier;
use saikuro_exec::{block_on, spawn, yield_now};
use saikuro_transport::memory::MemoryTransport;
use saikuro_transport::traits::{Transport, TransportReceiver, TransportSender};
use std::sync::Arc;

// COMPLIANCE TEST SUITE

/// Run the full compliance suite against a transport pair factory.
///
/// `factory` must return a connected `(transport_a, transport_b)` pair
/// where bytes sent on `a` arrive on `b` and vice versa.
pub fn run_transport_compliance<F>(factory: F)
where
    F: Fn() -> (MemoryTransport, MemoryTransport),
{
    send_receive_single_frame(factory());
    multiple_frames_in_order(factory());
    recv_returns_none_when_sender_dropped(factory());
    send_fails_when_receiver_dropped(factory());
    bidirectional_exchange(factory());
    empty_frame(factory());
    large_frame(factory());
    close_sender_signals_eof(factory());
    concurrent_send_receive(factory());
    sender_receiver_independent_lifecycles(factory());
    many_sequential_transports_correct(factory());
}

fn send_receive_single_frame(pair: (MemoryTransport, MemoryTransport)) {
    block_on(async {
        let (a, b) = pair;
        let (mut sender, _) = a.split();
        let (_, mut receiver) = b.split();

        let payload = Bytes::from_static(b"hello compliance");
        sender.send(payload.clone()).await.expect("send");
        let received = receiver.recv().await.expect("recv ok").expect("some frame");
        assert_eq!(received, payload);
    })
}

fn multiple_frames_in_order(pair: (MemoryTransport, MemoryTransport)) {
    block_on(async {
        let (a, b) = pair;
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
    })
}

fn recv_returns_none_when_sender_dropped(pair: (MemoryTransport, MemoryTransport)) {
    block_on(async {
        let (a, b) = pair;
        let (sender, _) = a.split();
        let (_, mut receiver) = b.split();
        drop(sender);
        yield_now().await;
        let result = receiver.recv().await.expect("recv should not error");
        assert!(result.is_none(), "expected None after sender dropped");
    })
}

fn send_fails_when_receiver_dropped(pair: (MemoryTransport, MemoryTransport)) {
    block_on(async {
        let (a, b) = pair;
        let (mut sender, _) = a.split();
        let (_, receiver) = b.split();
        drop(receiver);
        yield_now().await;
        let result = sender.send(Bytes::from_static(b"test")).await;
        assert!(result.is_err(), "send should fail with receiver dropped");
    })
}

fn bidirectional_exchange(pair: (MemoryTransport, MemoryTransport)) {
    block_on(async {
        let (a, b) = pair;
        let (mut a_tx, mut a_rx) = a.split();
        let (mut b_tx, mut b_rx) = b.split();

        a_tx.send(Bytes::from_static(b"ping")).await.unwrap();
        let ping = b_rx.recv().await.unwrap().unwrap();
        assert_eq!(ping, Bytes::from_static(b"ping"));

        b_tx.send(Bytes::from_static(b"pong")).await.unwrap();
        let pong = a_rx.recv().await.unwrap().unwrap();
        assert_eq!(pong, Bytes::from_static(b"pong"));
    })
}

fn empty_frame(pair: (MemoryTransport, MemoryTransport)) {
    block_on(async {
        let (a, b) = pair;
        let (mut sender, _) = a.split();
        let (_, mut receiver) = b.split();
        sender.send(Bytes::new()).await.unwrap();
        let got = receiver.recv().await.unwrap().unwrap();
        assert!(got.is_empty());
    })
}

fn large_frame(pair: (MemoryTransport, MemoryTransport)) {
    block_on(async {
        let (a, b) = pair;
        let (mut sender, _) = a.split();
        let (_, mut receiver) = b.split();
        let big = Bytes::from(vec![0xABu8; 1024 * 1024]);
        sender.send(big.clone()).await.unwrap();
        let got = receiver.recv().await.unwrap().unwrap();
        assert_eq!(got, big);
    })
}

fn close_sender_signals_eof(pair: (MemoryTransport, MemoryTransport)) {
    block_on(async {
        let (a, b) = pair;
        let (mut sender, _) = a.split();
        let (_, mut receiver) = b.split();

        sender.send(Bytes::from_static(b"last")).await.unwrap();
        sender.close().await.unwrap();
        drop(sender);

        let frame = receiver.recv().await.unwrap().unwrap();
        assert_eq!(frame, Bytes::from_static(b"last"));
        let eof = receiver.recv().await.unwrap();
        assert!(eof.is_none());
    })
}

fn concurrent_send_receive(pair: (MemoryTransport, MemoryTransport)) {
    block_on(async {
        let (a, b) = pair;
        let (mut sender, _) = a.split();
        let (_, mut receiver) = b.split();

        const N: usize = 100;
        let barrier = Arc::new(Barrier::new(2));

        let b2 = barrier.clone();
        let sender_task = spawn(async move {
            b2.wait().await;
            for i in 0..N {
                sender.send(Bytes::from(vec![i as u8])).await.unwrap();
            }
        });

        let recv_task = spawn(async move {
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
        assert_eq!(received, (0..N).map(|i| i as u8).collect::<Vec<_>>());
    })
}

fn sender_receiver_independent_lifecycles(pair: (MemoryTransport, MemoryTransport)) {
    block_on(async {
        let (a, b) = pair;
        // a's sender sends TO b's receiver.
        let (mut sender, _) = a.split();
        let (_, mut receiver) = b.split();
        // Drop the SENDER half, receiver should still get pending frames.
        sender.send(Bytes::from_static(b"pending")).await.unwrap();
        drop(sender);
        yield_now().await;
        let frame = receiver.recv().await.unwrap();
        assert_eq!(frame, Some(Bytes::from_static(b"pending")));
    })
}

fn many_sequential_transports_correct(pair: (MemoryTransport, MemoryTransport)) {
    block_on(async {
        let (a, b) = pair;
        let (mut sender, _) = a.split();
        let (_, mut receiver) = b.split();
        let payload = Bytes::from_static(b"sequential");
        sender.send(payload.clone()).await.unwrap();
        assert_eq!(receiver.recv().await.unwrap().unwrap(), payload);
    })
}

// RUN COMPLIANCE

#[test]
fn memory_transport_compliance() {
    run_transport_compliance(MemoryTransport::connected_pair);
}

#[test]
fn memory_transport_compliance_labeled() {
    run_transport_compliance(|| MemoryTransport::pair("compliance-a", "compliance-b"));
}
