use crate::common;
use crate::shared_test;
use crate::TestSuite;
use bytes::Bytes;
use saikuro_core::Arc;
use saikuro_exec::sync::Barrier;
use saikuro_exec::{block_on, spawn, yield_now};
use saikuro_transport::{MemoryTransport, Transport, TransportReceiver, TransportSender};

fn run_transport_compliance<F>(factory: F) -> Result<(), &'static str>
where
    F: Fn() -> (MemoryTransport, MemoryTransport),
{
    send_receive_single_frame(factory())?;
    multiple_frames_in_order(factory())?;
    recv_returns_none_when_sender_dropped(factory())?;
    send_fails_when_receiver_dropped(factory())?;
    bidirectional_exchange(factory())?;
    empty_frame(factory())?;
    close_sender_signals_eof(factory())?;
    concurrent_send_receive(factory())?;
    sender_receiver_independent_lifecycles(factory())?;
    many_sequential_transports_correct(factory())?;
    Ok(())
}

fn send_receive_single_frame(pair: (MemoryTransport, MemoryTransport)) -> Result<(), &'static str> {
    block_on(async {
        let (a, b) = pair;
        let (mut sender, _) = a.split();
        let (_, mut receiver) = b.split();

        let payload = Bytes::from_static(b"hello compliance");
        sender.send(payload.clone()).await.map_err(|_| "send")?;
        let received = receiver
            .recv()
            .await
            .map_err(|_| "recv ok")?
            .ok_or("some frame")?;
        assert_eq!(received, payload);
        Ok(())
    })
}

fn multiple_frames_in_order(pair: (MemoryTransport, MemoryTransport)) -> Result<(), &'static str> {
    block_on(async {
        let (a, b) = pair;
        let (mut sender, _) = a.split();
        let (_, mut receiver) = b.split();

        let frames: crate::Vec<Bytes> =
            (0u8..10).map(|i| Bytes::from(crate::vec![i; 16])).collect();
        for frame in &frames {
            sender.send(frame.clone()).await.map_err(|_| "send")?;
        }
        for expected in &frames {
            let got = receiver.recv().await.map_err(|_| "recv")?.ok_or("frame")?;
            assert_eq!(&got, expected);
        }
        Ok(())
    })
}

fn recv_returns_none_when_sender_dropped(
    pair: (MemoryTransport, MemoryTransport),
) -> Result<(), &'static str> {
    block_on(async {
        let (a, b) = pair;
        let (sender, _) = a.split();
        let (_, mut receiver) = b.split();
        drop(sender);
        yield_now().await;
        let result = receiver.recv().await.map_err(|_| "recv should not error")?;
        assert!(result.is_none(), "expected None after sender dropped");
        Ok(())
    })
}

fn send_fails_when_receiver_dropped(
    pair: (MemoryTransport, MemoryTransport),
) -> Result<(), &'static str> {
    block_on(async {
        let (a, b) = pair;
        let (mut sender, _) = a.split();
        let (_, receiver) = b.split();
        drop(receiver);
        yield_now().await;
        let result = sender.send(Bytes::from_static(b"test")).await;
        assert!(result.is_err(), "send should fail with receiver dropped");
        Ok(())
    })
}

fn bidirectional_exchange(pair: (MemoryTransport, MemoryTransport)) -> Result<(), &'static str> {
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
        Ok(())
    })
}

fn empty_frame(pair: (MemoryTransport, MemoryTransport)) -> Result<(), &'static str> {
    block_on(async {
        let (a, b) = pair;
        let (mut sender, _) = a.split();
        let (_, mut receiver) = b.split();
        sender.send(Bytes::new()).await.unwrap();
        let got = receiver.recv().await.unwrap().unwrap();
        assert!(got.is_empty());
        Ok(())
    })
}

fn large_frame(pair: (MemoryTransport, MemoryTransport)) -> Result<(), &'static str> {
    const BUFFER: usize = 1024 * 1024;
    crate::capacity::require_capacity(BUFFER)?;
    block_on(async {
        let (a, b) = pair;
        let (mut sender, _) = a.split();
        let (_, mut receiver) = b.split();
        let big = Bytes::from(crate::vec![0xABu8; BUFFER]);
        sender.send(big.clone()).await.unwrap();
        let got = receiver.recv().await.unwrap().unwrap();
        assert_eq!(got, big);
        Ok(())
    })
}

/// Standalone large-payload transport round-trip, kept out of the compliance bundle
/// so only this sub-check skips on chips with less than 1 MB of budget.
fn large_frame_round_trip() -> Result<(), &'static str> {
    let log = common::null_log();
    large_frame(MemoryTransport::connected_pair(log))
}

fn close_sender_signals_eof(pair: (MemoryTransport, MemoryTransport)) -> Result<(), &'static str> {
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
        Ok(())
    })
}

fn concurrent_send_receive(pair: (MemoryTransport, MemoryTransport)) -> Result<(), &'static str> {
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
                sender
                    .send(Bytes::from(crate::vec![i as u8]))
                    .await
                    .unwrap();
            }
        });

        let recv_task = spawn(async move {
            barrier.wait().await;
            let mut received = crate::Vec::with_capacity(N);
            for _ in 0..N {
                let frame = receiver.recv().await.unwrap().unwrap();
                received.push(frame[0]);
            }
            received
        });

        sender_task.await.unwrap();
        let received = recv_task.await.unwrap();
        assert_eq!(received, (0..N).map(|i| i as u8).collect::<crate::Vec<_>>());
        Ok(())
    })
}

fn sender_receiver_independent_lifecycles(
    pair: (MemoryTransport, MemoryTransport),
) -> Result<(), &'static str> {
    block_on(async {
        let (a, b) = pair;
        let (mut sender, _) = a.split();
        let (_, mut receiver) = b.split();
        sender.send(Bytes::from_static(b"pending")).await.unwrap();
        drop(sender);
        yield_now().await;
        let frame = receiver.recv().await.unwrap();
        assert_eq!(frame, Some(Bytes::from_static(b"pending")));
        Ok(())
    })
}

fn many_sequential_transports_correct(
    pair: (MemoryTransport, MemoryTransport),
) -> Result<(), &'static str> {
    block_on(async {
        let (a, b) = pair;
        let (mut sender, _) = a.split();
        let (_, mut receiver) = b.split();
        let payload = Bytes::from_static(b"sequential");
        sender.send(payload.clone()).await.unwrap();
        assert_eq!(receiver.recv().await.unwrap().unwrap(), payload);
        Ok(())
    })
}

pub fn register(suite: &mut TestSuite) {
    shared_test!(suite,
        "transport::memory_transport_compliance",
        memory_transport_compliance,
    );
    shared_test!(suite,
        "transport::memory_transport_compliance_labeled",
        memory_transport_compliance_labeled,
    );
    shared_test!(suite,
        "transport::large_frame_round_trip",
        large_frame_round_trip,
    );
}

fn memory_transport_compliance() -> Result<(), &'static str> {
    let log = common::null_log();
    run_transport_compliance(move || MemoryTransport::connected_pair(log.clone()))
}

fn memory_transport_compliance_labeled() -> Result<(), &'static str> {
    let log = common::null_log();
    run_transport_compliance(move || {
        MemoryTransport::pair("compliance-a", "compliance-b", log.clone())
    })
}
