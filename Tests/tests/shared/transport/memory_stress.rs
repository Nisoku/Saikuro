use crate::common;
use crate::shared_test;
use crate::TestSuite;
use bytes::Bytes;
use saikuro_core::Arc;
use saikuro_exec::sync::Barrier;
use saikuro_transport::{MemoryTransport, Transport, TransportReceiver, TransportSender};

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "transport::ten_thousand_frames_in_order",
        ten_thousand_frames_in_order,
    );
    shared_test!(
        suite,
        "transport::concurrent_bidirectional_stress",
        concurrent_bidirectional_stress,
    );
    shared_test!(
        suite,
        "transport::backpressure_sender_blocks_until_drain",
        backpressure_sender_blocks_until_drain,
    );
    shared_test!(
        suite,
        "transport::rapid_connect_disconnect_cycles",
        rapid_connect_disconnect_cycles,
    );
    shared_test!(
        suite,
        "transport::max_size_frame_just_under_limit",
        max_size_frame_just_under_limit,
    );
    shared_test!(
        suite,
        "transport::zero_length_frames_dont_confuse_ordering",
        zero_length_frames_dont_confuse_ordering,
    );
    shared_test!(
        suite,
        "transport::many_concurrent_senders_single_receiver",
        many_concurrent_senders_single_receiver,
    );
    shared_test!(
        suite,
        "transport::drop_receiver_while_sender_is_sending",
        drop_receiver_while_sender_is_sending,
    );
    shared_test!(
        suite,
        "transport::labels_do_not_cross_transports",
        labels_do_not_cross_transports,
    );
}

fn ten_thousand_frames_in_order() -> Result<(), &'static str> {
    saikuro_exec::block_on(async {
        let (a, b) = MemoryTransport::connected_pair(common::null_log());
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
        Ok(())
    })
}

fn concurrent_bidirectional_stress() -> Result<(), &'static str> {
    saikuro_exec::block_on(async {
        let (a, b) = MemoryTransport::connected_pair(common::null_log());
        let (mut a_tx, mut a_rx) = a.split();
        let (mut b_tx, mut b_rx) = b.split();

        let n = 500usize;
        let barrier = Arc::new(Barrier::new(2));

        let b1 = barrier.clone();
        let side_a = saikuro_exec::spawn(async move {
            b1.wait().await;
            for i in 0..n {
                a_tx.send(Bytes::from(crate::vec![i as u8])).await.unwrap();
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
                b_tx.send(frame).await.unwrap();
            }
        });

        side_a.await.unwrap();
        side_b.await.unwrap();
        Ok(())
    })
}

fn backpressure_sender_blocks_until_drain() -> Result<(), &'static str> {
    saikuro_exec::block_on(async {
        let (a, b) = MemoryTransport::connected_pair(common::null_log());
        let (mut sender, _) = a.split();
        let (_, mut receiver) = b.split();

        for _ in 0..256 {
            sender.send(Bytes::from_static(b"x")).await.unwrap();
        }

        let (ready_tx, ready_rx) = saikuro_exec::oneshot::channel();
        let (done_tx, done_rx) = saikuro_exec::oneshot::channel();
        let drainer = saikuro_exec::spawn(async move {
            for _ in 0..128 {
                receiver.recv().await.unwrap().unwrap();
            }
            let _ = ready_tx.send(());
            let _ = done_rx.await;
            drop(receiver);
        });

        ready_rx.await.unwrap();
        sender.send(Bytes::from_static(b"final")).await.unwrap();
        let _ = done_tx.send(());
        drainer.await.unwrap();
        Ok(())
    })
}

fn rapid_connect_disconnect_cycles() -> Result<(), &'static str> {
    saikuro_exec::block_on(async {
        for _ in 0..100 {
            let (a, b) = MemoryTransport::connected_pair(common::null_log());
            let (mut sender, _) = a.split();
            let (_, mut receiver) = b.split();

            sender.send(Bytes::from_static(b"round")).await.unwrap();
            let got = receiver.recv().await.unwrap().unwrap();
            assert_eq!(got, Bytes::from_static(b"round"));
        }
        Ok(())
    })
}

fn max_size_frame_just_under_limit() -> Result<(), &'static str> {
    const BUFFER: usize = 16 * 1024 * 1024;
    crate::capacity::require_capacity(BUFFER)?;
    saikuro_exec::block_on(async {
        let (a, b) = MemoryTransport::connected_pair(common::null_log());
        let (mut sender, _) = a.split();
        let (_, mut receiver) = b.split();

        let big = Bytes::from(crate::vec![0xFFu8; BUFFER]);
        sender.send(big.clone()).await.unwrap();
        let got = receiver.recv().await.unwrap().unwrap();
        assert_eq!(got.len(), BUFFER);
        assert_eq!(got, big);
        Ok(())
    })
}

fn zero_length_frames_dont_confuse_ordering() -> Result<(), &'static str> {
    saikuro_exec::block_on(async {
        let (a, b) = MemoryTransport::connected_pair(common::null_log());
        let (mut sender, _) = a.split();
        let (_, mut receiver) = b.split();

        sender.send(Bytes::new()).await.unwrap();
        sender.send(Bytes::from_static(b"after")).await.unwrap();

        let first = receiver.recv().await.unwrap().unwrap();
        assert!(first.is_empty(), "first frame should be empty");

        let second = receiver.recv().await.unwrap().unwrap();
        assert_eq!(second, Bytes::from_static(b"after"));
        Ok(())
    })
}

fn many_concurrent_senders_single_receiver() -> Result<(), &'static str> {
    saikuro_exec::block_on(async {
        let (a, b) = MemoryTransport::connected_pair(common::null_log());
        let (mut sender_base, _) = a.split();
        let (_, mut receiver) = b.split();

        let n = 50usize;
        let mut handles = crate::Vec::with_capacity(n);

        for i in 0..n {
            let (a_i, b_i) = MemoryTransport::connected_pair(common::null_log());
            let (mut tx_i, _) = a_i.split();
            let (_, mut rx_i) = b_i.split();
            handles.push(saikuro_exec::spawn(async move {
                tx_i.send(Bytes::from(crate::vec![i as u8])).await.unwrap();
                rx_i.recv().await.unwrap().unwrap()
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        sender_base.send(Bytes::from_static(b"done")).await.unwrap();
        let final_frame = receiver.recv().await.unwrap().unwrap();
        assert_eq!(final_frame, Bytes::from_static(b"done"));
        Ok(())
    })
}

fn drop_receiver_while_sender_is_sending() -> Result<(), &'static str> {
    saikuro_exec::block_on(async {
        let (a, b) = MemoryTransport::connected_pair(common::null_log());
        let (mut sender, _) = a.split();
        let (_, receiver) = b.split();

        let abort = saikuro_exec::spawn(async move {
            saikuro_exec::sleep(core::time::Duration::from_millis(5)).await;
            drop(receiver);
        });

        let mut receiver_dropped = false;
        for _ in 0..256 {
            if sender.send(Bytes::from_static(b"x")).await.is_err() {
                receiver_dropped = true;
                break;
            }
        }
        if !receiver_dropped {
            let _ = sender.send(Bytes::from_static(b"last")).await;
        }
        abort.await.unwrap();
        Ok(())
    })
}

fn labels_do_not_cross_transports() -> Result<(), &'static str> {
    saikuro_exec::block_on(async {
        let (a1, b1) = MemoryTransport::pair("sys-A", "sys-B", common::null_log());
        let (a2, b2) = MemoryTransport::pair("sys-C", "sys-D", common::null_log());

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
        Ok(())
    })
}
