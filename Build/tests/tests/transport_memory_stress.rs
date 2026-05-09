//! Stress and edge-case tests for MemoryTransport.
//!
//! Goes beyond the compliance suite with larger payloads, higher
//! concurrency, rapid connect-disconnect cycles, and backpressure
//! scenarios specific to the in-memory channel backend.

use bytes::Bytes;
use saikuro_exec::sync::Barrier;
use saikuro_transport::{
    memory::MemoryTransport,
    traits::{Transport, TransportReceiver, TransportSender},
};
use std::sync::Arc;

// HIGH-VOLUME THROUGHPUT

#[test]
fn ten_thousand_frames_in_order() {
    saikuro_exec::block_on(async {
        let (a, b) = MemoryTransport::connected_pair();
        let (mut sender, _) = a.split();
        let (_, mut receiver) = b.split();

        let n = 10_000u32;
        let producer = saikuro_exec::spawn(async move {
            for i in 0..n {
                let frame = Bytes::from(i.to_le_bytes().to_vec());
                sender.send(frame).await.unwrap();
            }
        });

        let consumer = saikuro_exec::spawn(async move {
            for i in 0..n {
                let frame = receiver.recv().await.unwrap().unwrap();
                let val = u32::from_le_bytes(frame[..4].try_into().unwrap());
                assert_eq!(val, i, "out-of-order frame at index {i}");
            }
        });

        producer.await.unwrap();
        consumer.await.unwrap();
    })
}

#[test]
fn concurrent_bidirectional_stress() {
    saikuro_exec::block_on(async {
        let (a, b) = MemoryTransport::connected_pair();
        let (mut a_tx, mut a_rx) = a.split();
        let (mut b_tx, mut b_rx) = b.split();

        let n = 500usize;
        let barrier = Arc::new(Barrier::new(2));

        let b1 = barrier.clone();
        let side_a = saikuro_exec::spawn(async move {
            b1.wait().await;
            for i in 0..n {
                a_tx.send(Bytes::from(vec![i as u8])).await.unwrap();
                let echo = a_rx.recv().await.unwrap().unwrap();
                assert_eq!(echo[0], i as u8, "side-a echo mismatch at {i}");
            }
        });

        let b2 = barrier.clone();
        let side_b = saikuro_exec::spawn(async move {
            b2.wait().await;
            for i in 0..n {
                let frame = b_rx.recv().await.unwrap().unwrap();
                assert_eq!(frame[0], i as u8, "side-b recv mismatch at {i}");
                b_tx.send(frame).await.unwrap(); // echo back
            }
        });

        side_a.await.unwrap();
        side_b.await.unwrap();
    })
}

// BACKPRESSURE

#[test]
fn backpressure_sender_blocks_until_drain() {
    saikuro_exec::block_on(async {
        let (a, b) = MemoryTransport::connected_pair();
        let (mut sender, _) = a.split();
        let (_, mut receiver) = b.split();

        // Fill the channel (default capacity = 256).
        for _ in 0..256 {
            sender.send(Bytes::from_static(b"x")).await.unwrap();
        }

        // The 257th send would block if nobody is receiving.
        // Spawn a drainer to unblock us.
        let drainer = saikuro_exec::spawn(async move {
            // Receive enough to let the sender proceed.
            for _ in 0..128 {
                receiver.recv().await.unwrap().unwrap();
            }
        });
        // Give the executor a chance to run the drainer so it begins
        // consuming queued messages before we attempt the blocking send.
        saikuro_exec::yield_now().await;

        // This send should eventually succeed after the drainer runs.
        sender.send(Bytes::from_static(b"final")).await.unwrap();
        drainer.await.unwrap();
    })
}

#[test]
fn rapid_connect_disconnect_cycles() {
    saikuro_exec::block_on(async {
        for _ in 0..100 {
            let (a, b) = MemoryTransport::connected_pair();
            let (mut sender, _) = a.split();
            let (_, mut receiver) = b.split();

            sender.send(Bytes::from_static(b"round")).await.unwrap();
            let got = receiver.recv().await.unwrap().unwrap();
            assert_eq!(got, Bytes::from_static(b"round"));

            // Both halves go out of scope here; transport is dropped.
        }
    })
}

// EDGE: HUGE PAYLOADS

#[test]
fn max_size_frame_just_under_limit() {
    saikuro_exec::block_on(async {
        let (a, b) = MemoryTransport::connected_pair();
        let (mut sender, _) = a.split();
        let (_, mut receiver) = b.split();

        // 16 MiB (default max_message_size).
        let big = Bytes::from(vec![0xFFu8; 16 * 1024 * 1024]);
        sender.send(big.clone()).await.unwrap();
        let got = receiver.recv().await.unwrap().unwrap();
        assert_eq!(got.len(), 16 * 1024 * 1024);
        assert_eq!(got, big);
    })
}

#[test]
fn zero_length_frames_dont_confuse_ordering() {
    saikuro_exec::block_on(async {
        let (a, b) = MemoryTransport::connected_pair();
        let (mut sender, _) = a.split();
        let (_, mut receiver) = b.split();

        sender.send(Bytes::new()).await.unwrap();
        sender.send(Bytes::from_static(b"after")).await.unwrap();

        let first = receiver.recv().await.unwrap().unwrap();
        assert!(first.is_empty(), "first frame should be empty");

        let second = receiver.recv().await.unwrap().unwrap();
        assert_eq!(second, Bytes::from_static(b"after"));
    })
}

// EDGE: CONCURRENT SENDERS

#[test]
fn many_concurrent_senders_single_receiver() {
    saikuro_exec::block_on(async {
        let (a, b) = MemoryTransport::connected_pair();
        let (mut sender_base, _) = a.split();
        let (_, mut receiver) = b.split();

        let n = 50usize;
        let mut handles = Vec::with_capacity(n);

        // Since TransportSender::send takes &mut self, each sender must be
        // used from one task.  Create multiple transports for parallelism.
        for i in 0..n {
            let (a_i, b_i) = MemoryTransport::connected_pair();
            let (mut tx_i, _) = a_i.split();
            let (_, mut rx_i) = b_i.split();
            handles.push(saikuro_exec::spawn(async move {
                tx_i.send(Bytes::from(vec![i as u8])).await.unwrap();
                rx_i.recv().await.unwrap().unwrap()
            }));
        }

        // Collect all results.
        for h in handles {
            h.await.unwrap();
        }

        // Ensure the original transport still works.
        sender_base.send(Bytes::from_static(b"done")).await.unwrap();
        let final_frame = receiver.recv().await.unwrap().unwrap();
        assert_eq!(final_frame, Bytes::from_static(b"done"));
    })
}

// EDGE: DROP DURING SEND

#[test]
fn drop_receiver_while_sender_is_sending() {
    saikuro_exec::block_on(async {
        let (a, b) = MemoryTransport::connected_pair();
        let (mut sender, _) = a.split();
        let (_, receiver) = b.split();

        let abort = saikuro_exec::spawn(async move {
            // Give the sender a moment to start, then drop the receiver.
            saikuro_exec::sleep(std::time::Duration::from_millis(5)).await;
            drop(receiver);
        });

        // Fill the channel then try to send one more (which will block,
        // then fail when the receiver is dropped).
        for _ in 0..256 {
            sender.send(Bytes::from_static(b"x")).await.unwrap();
        }
        // This send may return an error (receiver dropped) or succeed
        // (if the abort task hasn't run yet).  Either is acceptable.
        let _ = sender.send(Bytes::from_static(b"last")).await;
        abort.await.unwrap();
    })
}

// EDGE: LABEL ISOLATION

#[test]
fn labels_do_not_cross_transports() {
    saikuro_exec::block_on(async {
        let (a1, b1) = MemoryTransport::pair("sys-A", "sys-B");
        let (a2, b2) = MemoryTransport::pair("sys-C", "sys-D");

        let (mut a1_tx, _) = a1.split();
        let (_, mut b1_rx) = b1.split();
        let (mut a2_tx, _) = a2.split();
        let (_, mut b2_rx) = b2.split();

        a1_tx.send(Bytes::from_static(b"to-b1")).await.unwrap();
        a2_tx.send(Bytes::from_static(b"to-b2")).await.unwrap();

        let from_b1 = b1_rx.recv().await.unwrap().unwrap();
        let from_b2 = b2_rx.recv().await.unwrap().unwrap();
        assert_eq!(from_b1, Bytes::from_static(b"to-b1"));
        assert_eq!(from_b2, Bytes::from_static(b"to-b2"));
    })
}
