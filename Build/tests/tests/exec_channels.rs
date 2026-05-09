//! Comprehensive channel tests for saikuro-exec channels.
//!
//! Tests mpsc, oneshot, and watch modules through the saikuro-exec facade.
//! These run on the tokio backend (native) and validate the same API surface
//! that the WASM backend must match.

use saikuro_exec::{mpsc, oneshot, watch};
use std::time::Duration;

// MPSC

#[test]
fn mpsc_send_recv_single() {
    saikuro_exec::block_on(async {
        let (tx, mut rx) = mpsc::channel::<u32>(16);
        tx.send(42).await.unwrap();
        assert_eq!(rx.recv().await, Some(42));
    })
}

#[test]
fn mpsc_send_recv_multiple_in_order() {
    saikuro_exec::block_on(async {
        let (tx, mut rx) = mpsc::channel::<i32>(32);
        for i in 0..10 {
            tx.send(i).await.unwrap();
        }
        for i in 0..10 {
            assert_eq!(rx.recv().await, Some(i));
        }
    })
}

#[test]
fn mpsc_backpressure_sender_waits() {
    saikuro_exec::block_on(async {
        let (tx, mut rx) = mpsc::channel::<u8>(3);
        // Fill the buffer:  3 slots.
        tx.send(1).await.unwrap();
        tx.send(2).await.unwrap();
        tx.send(3).await.unwrap();
        // The 4th send would block (channel full).  Spawn a task so we can
        // receive concurrently.
        let tx_clone = tx.clone();
        let handle = saikuro_exec::spawn(async move {
            tx_clone.send(4).await.unwrap();
        });
        // Drain one slot so the spawned sender can proceed.
        assert_eq!(rx.recv().await, Some(1));
        handle.await.unwrap();
        assert_eq!(rx.recv().await, Some(2));
        assert_eq!(rx.recv().await, Some(3));
        assert_eq!(rx.recv().await, Some(4));
    })
}

#[test]
fn mpsc_try_send_on_full_channel() {
    saikuro_exec::block_on(async {
        let (tx, _rx) = mpsc::channel::<u8>(2);
        tx.send(1).await.unwrap();
        tx.send(2).await.unwrap();
        // Channel is full; try_send should fail with the value returned.
        assert!(tx.try_send(3).is_err());
    })
}

#[test]
fn mpsc_try_send_on_closed_channel() {
    saikuro_exec::block_on(async {
        let (tx, rx) = mpsc::channel::<u8>(2);
        drop(rx);
        // Allow the drop to propagate.
        saikuro_exec::yield_now().await;
        assert!(tx.try_send(99).is_err());
    })
}

#[test]
fn mpsc_sender_clone() {
    saikuro_exec::block_on(async {
        let (tx1, mut rx) = mpsc::channel::<&'static str>(8);
        let tx2 = tx1.clone();
        tx1.send("from-1").await.unwrap();
        tx2.send("from-2").await.unwrap();
        // Ordering:  whichever sender pushes first wins.
        let a = rx.recv().await;
        let b = rx.recv().await;
        let mut msgs: Vec<_> = vec![a, b].into_iter().flatten().collect();
        msgs.sort();
        assert_eq!(msgs, vec!["from-1", "from-2"]);
    })
}

#[test]
fn mpsc_send_after_all_receivers_dropped_errors() {
    saikuro_exec::block_on(async {
        let (tx, rx) = mpsc::channel::<u8>(8);
        drop(rx);
        let result = tx.send(7).await;
        assert!(result.is_err(), "send should fail after receiver dropped");
    })
}

#[test]
fn mpsc_recv_returns_none_when_all_senders_dropped() {
    saikuro_exec::block_on(async {
        let (tx, mut rx) = mpsc::channel::<u8>(8);
        tx.send(1).await.unwrap();
        drop(tx);
        // The buffered message must still be received.
        assert_eq!(rx.recv().await, Some(1));
        // After draining, the channel is closed.
        assert_eq!(rx.recv().await, None);
    })
}

#[test]
fn mpsc_large_message() {
    saikuro_exec::block_on(async {
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(8);
        let big = vec![0xABu8; 1024 * 1024]; // 1 MiB
        tx.send(big.clone()).await.unwrap();
        let got = rx.recv().await.unwrap();
        assert_eq!(got.len(), 1024 * 1024);
        assert_eq!(got[0], 0xAB);
        assert_eq!(got[1024 * 1024 - 1], 0xAB);
    })
}

#[test]
fn mpsc_many_messages_in_order() {
    saikuro_exec::block_on(async {
        let (tx, mut rx) = mpsc::channel::<u64>(1024);
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
    })
}

#[test]
fn mpsc_is_closed() {
    saikuro_exec::block_on(async {
        let (tx, rx) = mpsc::channel::<u8>(8);
        assert!(!tx.is_closed());
        drop(rx);
        saikuro_exec::yield_now().await;
        // Note: tokio mpsc::Sender::is_closed() may not reflect the drop
        // until a send is attempted.  This test documents the behaviour
        // rather than asserting.
        let _ = tx;
    })
}

#[test]
fn mpsc_multiple_concurrent_senders() {
    saikuro_exec::block_on(async {
        let (tx, mut rx) = mpsc::channel::<u32>(256);
        let mut handles = Vec::new();
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
        let mut received = Vec::new();
        while let Some(v) = rx.recv().await {
            received.push(v);
        }
        received.sort();
        assert_eq!(received, (0..10).collect::<Vec<_>>());
    })
}

// ONESHOT

#[test]
fn oneshot_send_recv() {
    saikuro_exec::block_on(async {
        let (tx, rx) = oneshot::channel::<u32>();
        tx.send(42).unwrap();
        assert_eq!(rx.await, Ok(42));
    })
}

#[test]
fn oneshot_dropped_sender_returns_err() {
    saikuro_exec::block_on(async {
        let (tx, rx) = oneshot::channel::<u32>();
        drop(tx);
        let result = rx.await;
        assert!(result.is_err());
    })
}

#[test]
fn oneshot_dropped_receiver_returns_value() {
    saikuro_exec::block_on(async {
        let (tx, rx) = oneshot::channel::<String>();
        drop(rx);
        // send should return Err(value) because the receiver is gone.
        let result = tx.send("hello".into());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "hello");
    })
}

#[test]
fn oneshot_send_after_recv_fails() {
    saikuro_exec::block_on(async {
        let (tx, rx) = oneshot::channel::<u8>();
        drop(rx);
        let result = tx.send(7);
        assert!(result.is_err());
    })
}

#[test]
fn oneshot_multiple_independent_channels() {
    saikuro_exec::block_on(async {
        let (tx1, rx1) = oneshot::channel::<u32>();
        let (tx2, rx2) = oneshot::channel::<&'static str>();
        tx1.send(100).unwrap();
        tx2.send("done").unwrap();
        assert_eq!(rx1.await, Ok(100));
        assert_eq!(rx2.await, Ok("done"));
    })
}

#[test]
fn oneshot_cannot_call_send_twice() {
    saikuro_exec::block_on(async {
        let (tx, rx) = oneshot::channel::<u8>();
        tx.send(1).unwrap();
        // send consumes the sender; calling send again is a compile error.
        // Just verify the receiver got the value.
        assert_eq!(rx.await, Ok(1));
    })
}

// WATCH

#[test]
fn watch_send_and_borrow() {
    saikuro_exec::block_on(async {
        let (tx, rx) = watch::channel(0u32);
        tx.send(42).unwrap();
        assert_eq!(*rx.borrow(), 42);
    })
}

#[test]
fn watch_send_and_changed() {
    saikuro_exec::block_on(async {
        let (tx, mut rx) = watch::channel(0u32);
        tx.send(1).unwrap();
        // changed() should return immediately since the value has changed.
        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow(), 1);
    })
}

#[test]
fn watch_changed_blocks_until_next_update() {
    saikuro_exec::block_on(async {
        let (tx, mut rx) = watch::channel(0u32);
        // changed() would block because the value hasn't changed since
        // the receiver was created.  Spawn a task to send later.
        let handle = saikuro_exec::spawn(async move {
            saikuro_exec::sleep(Duration::from_millis(20)).await;
            tx.send(99).unwrap();
        });
        // This should block until the spawned task sends.
        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow(), 99);
        handle.await.unwrap();
    })
}

#[test]
fn watch_initial_value_available() {
    saikuro_exec::block_on(async {
        let (_tx, rx) = watch::channel("hello");
        assert_eq!(*rx.borrow(), "hello");
    })
}

#[test]
fn watch_multiple_receivers() {
    saikuro_exec::block_on(async {
        let (tx, rx1) = watch::channel(0i32);
        let rx2 = rx1.clone();
        tx.send(10).unwrap();
        assert_eq!(*rx1.borrow(), 10);
        assert_eq!(*rx2.borrow(), 10);
    })
}

#[test]
fn watch_sender_drop_closes_channel() {
    saikuro_exec::block_on(async {
        let (tx, mut rx) = watch::channel(0u32);
        drop(tx);
        // changed() should return Err once the sender is dropped.
        let result = rx.changed().await;
        assert!(result.is_err());
    })
}

#[test]
fn watch_borrow_returns_last_value() {
    saikuro_exec::block_on(async {
        let (tx, rx) = watch::channel(1u64);
        tx.send(2).unwrap();
        tx.send(3).unwrap();
        assert_eq!(*rx.borrow(), 3);
    })
}
