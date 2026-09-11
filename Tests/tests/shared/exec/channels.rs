use crate::shared_test;
use crate::TestSuite;
use portable_atomic::{AtomicBool, Ordering};
use saikuro_core::Arc;
use saikuro_exec::{mpsc, oneshot, watch};

fn capacity(value: usize) -> saikuro_exec::ChannelCapacity {
    saikuro_exec::ChannelCapacity::try_from(value).expect("test channel capacity must be valid")
}

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "exec::channel_capacity_enforces_shared_backend_bounds",
        channel_capacity_enforces_shared_backend_bounds,
    );
    shared_test!(suite, "exec::mpsc_send_recv_single", mpsc_send_recv_single);
    shared_test!(
        suite,
        "exec::mpsc_send_recv_multiple_in_order",
        mpsc_send_recv_multiple_in_order,
    );
    shared_test!(
        suite,
        "exec::mpsc_backpressure_sender_waits",
        mpsc_backpressure_sender_waits,
    );
    shared_test!(
        suite,
        "exec::mpsc_try_send_on_full_channel",
        mpsc_try_send_on_full_channel,
    );
    shared_test!(
        suite,
        "exec::mpsc_try_send_on_closed_channel",
        mpsc_try_send_on_closed_channel,
    );
    shared_test!(suite, "exec::mpsc_sender_clone", mpsc_sender_clone);
    shared_test!(
        suite,
        "exec::mpsc_send_after_all_receivers_dropped_errors",
        mpsc_send_after_all_receivers_dropped_errors,
    );
    shared_test!(
        suite,
        "exec::mpsc_recv_returns_none_when_all_senders_dropped",
        mpsc_recv_returns_none_when_all_senders_dropped,
    );
    shared_test!(
        suite,
        "exec::mpsc_drop_after_blocked_recv_releases_task",
        mpsc_drop_after_blocked_recv_releases_task,
    );
    shared_test!(suite, "exec::mpsc_large_message", mpsc_large_message);
    shared_test!(
        suite,
        "exec::mpsc_many_messages_in_order",
        mpsc_many_messages_in_order,
    );
    shared_test!(suite, "exec::mpsc_is_closed", mpsc_is_closed);
    shared_test!(
        suite,
        "exec::mpsc_multiple_concurrent_senders",
        mpsc_multiple_concurrent_senders,
    );
    shared_test!(suite, "exec::oneshot_send_recv", oneshot_send_recv);
    shared_test!(
        suite,
        "exec::oneshot_dropped_sender_returns_err",
        oneshot_dropped_sender_returns_err,
    );
    shared_test!(
        suite,
        "exec::oneshot_dropped_receiver_returns_value",
        oneshot_dropped_receiver_returns_value,
    );
    shared_test!(
        suite,
        "exec::oneshot_send_after_recv_fails",
        oneshot_send_after_recv_fails,
    );
    shared_test!(
        suite,
        "exec::oneshot_multiple_independent_channels",
        oneshot_multiple_independent_channels,
    );
    shared_test!(
        suite,
        "exec::oneshot_cannot_call_send_twice",
        oneshot_cannot_call_send_twice,
    );
    shared_test!(suite, "exec::watch_send_and_borrow", watch_send_and_borrow);
    shared_test!(
        suite,
        "exec::watch_send_and_changed",
        watch_send_and_changed
    );
    shared_test!(
        suite,
        "exec::watch_changed_blocks_until_next_update",
        watch_changed_blocks_until_next_update,
    );
    shared_test!(
        suite,
        "exec::watch_initial_value_available",
        watch_initial_value_available,
    );
    shared_test!(
        suite,
        "exec::watch_multiple_receivers",
        watch_multiple_receivers
    );
    shared_test!(
        suite,
        "exec::watch_sender_drop_closes_channel",
        watch_sender_drop_closes_channel,
    );
    shared_test!(
        suite,
        "exec::watch_borrow_returns_last_value",
        watch_borrow_returns_last_value,
    );
}

fn channel_capacity_enforces_shared_backend_bounds() -> Result<(), &'static str> {
    use saikuro_exec::ChannelCapacity;

    assert_eq!(ChannelCapacity::MIN.get(), 1);
    assert_eq!(ChannelCapacity::MAX.get(), 256);
    assert_eq!(ChannelCapacity::try_from(1), Ok(ChannelCapacity::MIN));
    assert_eq!(ChannelCapacity::try_from(256), Ok(ChannelCapacity::MAX));
    assert_eq!(ChannelCapacity::try_from(0).unwrap_err().value(), 0);
    assert_eq!(ChannelCapacity::try_from(257).unwrap_err().value(), 257);
    Ok(())
}

fn mpsc_send_recv_single() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, mut rx) = mpsc::channel::<u32>(capacity(16));
        tx.send(42).await.unwrap();
        assert_eq!(rx.recv().await, Some(42));
        Ok(())
    })
}

fn mpsc_send_recv_multiple_in_order() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, mut rx) = mpsc::channel::<i32>(capacity(32));
        for i in 0..10 {
            tx.send(i).await.unwrap();
        }
        for i in 0..10 {
            assert_eq!(rx.recv().await, Some(i));
        }
        Ok(())
    })
}

fn mpsc_backpressure_sender_waits() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, mut rx) = mpsc::channel::<u8>(capacity(3));
        tx.send(1).await.unwrap();
        tx.send(2).await.unwrap();
        tx.send(3).await.unwrap();
        let tx_clone = tx.clone();
        let done = Arc::new(AtomicBool::new(false));
        let done_clone = done.clone();
        let handle = saikuro_exec::spawn(async move {
            tx_clone.send(4).await.unwrap();
            done_clone.store(true, Ordering::Release);
        });
        assert!(!done.load(Ordering::Acquire));
        assert_eq!(rx.recv().await, Some(1));
        handle.await.unwrap();
        assert!(done.load(Ordering::Acquire));
        assert_eq!(rx.recv().await, Some(2));
        assert_eq!(rx.recv().await, Some(3));
        assert_eq!(rx.recv().await, Some(4));
        Ok(())
    })
}

fn mpsc_try_send_on_full_channel() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, _rx) = mpsc::channel::<u8>(capacity(2));
        tx.send(1).await.unwrap();
        tx.send(2).await.unwrap();
        assert!(tx.try_send(3).is_err());
        Ok(())
    })
}

fn mpsc_try_send_on_closed_channel() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, rx) = mpsc::channel::<u8>(capacity(2));
        drop(rx);
        saikuro_exec::yield_now().await;
        assert!(tx.try_send(99).is_err());
        Ok(())
    })
}

fn mpsc_sender_clone() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx1, mut rx) = mpsc::channel::<&'static str>(capacity(8));
        let tx2 = tx1.clone();
        tx1.send("from-1").await.unwrap();
        tx2.send("from-2").await.unwrap();
        let a = rx.recv().await;
        let b = rx.recv().await;
        let mut msgs: crate::Vec<_> = crate::vec![a, b].into_iter().flatten().collect();
        msgs.sort();
        assert_eq!(msgs, crate::vec!["from-1", "from-2"]);
        Ok(())
    })
}

fn mpsc_send_after_all_receivers_dropped_errors() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, rx) = mpsc::channel::<u8>(capacity(8));
        drop(rx);
        let result = tx.send(7).await;
        assert!(result.is_err(), "send should fail after receiver dropped");
        Ok(())
    })
}

fn mpsc_recv_returns_none_when_all_senders_dropped() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, mut rx) = mpsc::channel::<u8>(capacity(8));
        tx.send(1).await.unwrap();
        drop(tx);
        assert_eq!(rx.recv().await, Some(1));
        assert_eq!(rx.recv().await, None);
        Ok(())
    })
}

fn mpsc_drop_after_blocked_recv_releases_task() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, mut rx) = mpsc::channel::<u8>(capacity(4));
        let waiter = saikuro_exec::spawn(async move {
            let mut got = crate::Vec::new();
            while let Some(v) = rx.recv().await {
                got.push(v);
            }
            got
        });

        tx.send(7).await.unwrap();
        // `waiter` is now blocked inside `rx.recv()`
        drop(tx);
        let got = waiter.await.map_err(|_| "waiter join failed")?;
        Ok(())
    })
}

fn mpsc_large_message() -> Result<(), &'static str> {
    const BUFFER: usize = 1024 * 1024;
    crate::capacity::require_capacity(BUFFER)?;
    crate::block_on(async {
        let (tx, mut rx) = mpsc::channel::<crate::Vec<u8>>(capacity(8));
        let big = crate::vec![0xABu8; BUFFER];
        tx.send(big.clone()).await.unwrap();
        let got = rx.recv().await.unwrap();
        assert_eq!(got.len(), BUFFER);
        assert_eq!(got[0], 0xAB);
        assert_eq!(got[BUFFER - 1], 0xAB);
        Ok(())
    })
}

fn mpsc_many_messages_in_order() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, mut rx) = mpsc::channel::<u64>(saikuro_exec::ChannelCapacity::MAX);
        let n = 5000u64;
        let tx_clone = tx.clone();
        let producer = saikuro_exec::spawn(async move {
            for i in 0..n {
                tx_clone.send(i).await.unwrap();
            }
        });
        let consumer = saikuro_exec::spawn(async move {
            for i in 0..n {
                assert_eq!(rx.recv().await, Some(i), "out of order at {i}");
            }
        });
        drop(tx);
        producer.await.unwrap();
        consumer.await.unwrap();
        Ok(())
    })
}

fn mpsc_is_closed() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, rx) = mpsc::channel::<u8>(capacity(8));
        assert!(!tx.is_closed());
        drop(rx);
        saikuro_exec::yield_now().await;
        assert!(tx.is_closed());
        assert!(tx.try_send(0).is_err());
        Ok(())
    })
}

fn mpsc_multiple_concurrent_senders() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, mut rx) = mpsc::channel::<u32>(saikuro_exec::ChannelCapacity::MAX);
        let mut handles = crate::Vec::new();
        for i in 0..10 {
            let t = tx.clone();
            handles.push(saikuro_exec::spawn(async move {
                t.send(i).await.unwrap();
            }));
        }
        drop(tx);
        for h in handles {
            h.await.unwrap();
        }
        let mut received = crate::Vec::new();
        while let Some(v) = rx.recv().await {
            received.push(v);
        }
        received.sort();
        assert_eq!(received, (0..10).collect::<crate::Vec<_>>());
        Ok(())
    })
}

fn oneshot_send_recv() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, rx) = oneshot::channel::<u32>();
        tx.send(42).unwrap();
        assert_eq!(rx.await, Ok(42));
        Ok(())
    })
}

fn oneshot_dropped_sender_returns_err() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, rx) = oneshot::channel::<u32>();
        drop(tx);
        let result = rx.await;
        assert!(result.is_err());
        Ok(())
    })
}

fn oneshot_dropped_receiver_returns_value() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, rx) = oneshot::channel::<crate::String>();
        drop(rx);
        let result = tx.send("hello".into());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "hello");
        Ok(())
    })
}

fn oneshot_send_after_recv_fails() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, rx) = oneshot::channel::<u8>();
        drop(rx);
        let result = tx.send(7);
        assert!(result.is_err());
        Ok(())
    })
}

fn oneshot_multiple_independent_channels() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx1, rx1) = oneshot::channel::<u32>();
        let (tx2, rx2) = oneshot::channel::<&'static str>();
        tx1.send(100).unwrap();
        tx2.send("done").unwrap();
        assert_eq!(rx1.await, Ok(100));
        assert_eq!(rx2.await, Ok("done"));
        Ok(())
    })
}

fn oneshot_cannot_call_send_twice() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, rx) = oneshot::channel::<u8>();
        tx.send(1).unwrap();
        assert_eq!(rx.await, Ok(1));
        Ok(())
    })
}

fn watch_send_and_borrow() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, rx) = watch::channel(0u32);
        tx.send(42).unwrap();
        assert_eq!(rx.borrow(), 42);
        Ok(())
    })
}

fn watch_send_and_changed() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, mut rx) = watch::channel(0u32);
        tx.send(1).unwrap();
        rx.changed().await.unwrap();
        assert_eq!(rx.borrow(), 1);
        Ok(())
    })
}

fn watch_changed_blocks_until_next_update() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, mut rx) = watch::channel(0u32);
        let handle = saikuro_exec::spawn(async move {
            saikuro_exec::sleep(core::time::Duration::from_millis(20)).await;
            tx.send(99).unwrap();
        });
        rx.changed().await.unwrap();
        assert_eq!(rx.borrow(), 99);
        handle.await.unwrap();
        Ok(())
    })
}

fn watch_initial_value_available() -> Result<(), &'static str> {
    crate::block_on(async {
        let (_tx, rx) = watch::channel("hello");
        assert_eq!(rx.borrow(), "hello");
        Ok(())
    })
}

fn watch_multiple_receivers() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, rx1) = watch::channel(0i32);
        let rx2 = rx1.clone();
        tx.send(10).unwrap();
        assert_eq!(rx1.borrow(), 10);
        assert_eq!(rx2.borrow(), 10);
        Ok(())
    })
}

fn watch_sender_drop_closes_channel() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, mut rx) = watch::channel(0u32);
        drop(tx);
        let result = rx.changed().await;
        assert!(result.is_err());
        Ok(())
    })
}

fn watch_borrow_returns_last_value() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, rx) = watch::channel(1u64);
        tx.send(2).unwrap();
        tx.send(3).unwrap();
        assert_eq!(rx.borrow(), 3);
        Ok(())
    })
}
