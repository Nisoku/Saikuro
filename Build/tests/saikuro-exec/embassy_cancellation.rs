#![cfg(feature = "embassy-test")]

use std::future::Future;
use std::pin::Pin;
use std::task::Poll;
use std::time::Duration;

use futures::future::poll_fn;
use futures_executor::block_on;
use saikuro_exec::{mpsc, oneshot, sync, watch, ChannelCapacity};

/// Poll `fut` once with the surrounding executor's waker and assert it is
/// still pending.  The future parks exactly like an `.await` would, so a later
/// external event that wakes it is observable.
async fn assert_pending<F: Future + Unpin>(fut: &mut F) {
    poll_fn(|cx| {
        assert!(
            Pin::new(&mut *fut).poll(cx).is_pending(),
            "expected the future to be pending"
        );
        Poll::Ready(())
    })
    .await;
}

/// Await `fut` with a fail-on-timeout guard.
async fn guarded<F: Future>(fut: F) -> F::Output {
    saikuro_exec::timeout(Duration::from_secs(5), fut)
        .await
        .expect("test future timed out")
}

fn capacity(n: usize) -> ChannelCapacity {
    ChannelCapacity::new(n).expect("valid capacity")
}

#[test]
fn mpsc_sender_blocked_on_full_errors_when_receiver_dropped() {
    block_on(async {
        let (tx, rx) = mpsc::channel::<u32>(capacity(2));
        tx.send(1).await.expect("send first value");
        tx.send(2).await.expect("send second value");

        let mut send = Box::pin(tx.send(3));
        assert_pending(&mut send).await;

        drop(rx);

        let err = guarded(send).await.expect_err("receiver was dropped");
        assert_eq!(err.0, 3, "the undelivered value is returned");
    });
}

#[test]
fn mpsc_sender_blocked_on_full_completes_when_capacity_frees() {
    block_on(async {
        let (tx, mut rx) = mpsc::channel::<u32>(capacity(2));
        tx.send(1).await.expect("send first value");
        tx.send(2).await.expect("send second value");

        let mut send = Box::pin(tx.send(3));
        assert_pending(&mut send).await;

        assert_eq!(rx.recv().await, Some(1));
        guarded(send)
            .await
            .expect("sender proceeds once a slot frees");
        assert_eq!(rx.recv().await, Some(2));
        assert_eq!(rx.recv().await, Some(3));
    });
}

#[test]
fn mpsc_receiver_blocked_on_empty_returns_none_when_senders_dropped() {
    block_on(async {
        let (tx, mut rx) = mpsc::channel::<u32>(capacity(2));

        let mut recv = Box::pin(rx.recv());
        assert_pending(&mut recv).await;

        drop(tx);

        assert_eq!(guarded(recv).await, None, "channel closes with senders");
    });
}

#[test]
fn mpsc_receiver_cancelled_then_resumed_receives_sent_value() {
    block_on(async {
        let (tx, mut rx) = mpsc::channel::<u32>(capacity(2));

        {
            let mut recv = Box::pin(rx.recv());
            assert_pending(&mut recv).await;
            // Cancel the blocked receiver; its waker registration goes stale.
        }

        tx.send(7).await.expect("send after cancellation");
        assert_eq!(rx.recv().await, Some(7));
    });
}

#[test]
fn mpsc_cancelled_blocked_sender_does_not_corrupt_channel() {
    block_on(async {
        let (tx, mut rx) = mpsc::channel::<u32>(capacity(2));
        tx.send(1).await.expect("send first value");
        tx.send(2).await.expect("send second value");

        {
            let mut send = Box::pin(tx.send(3));
            assert_pending(&mut send).await;
            // Cancel the blocked sender; the undelivered value drops with it.
        }

        assert_eq!(rx.recv().await, Some(1));
        tx.send(4).await.expect("channel still accepts sends");
        assert_eq!(rx.recv().await, Some(2));
        assert_eq!(rx.recv().await, Some(4));
    });
}

#[test]
fn oneshot_receiver_pending_completes_when_sent() {
    block_on(async {
        let (tx, mut rx) = oneshot::channel();
        assert_pending(&mut rx).await;

        tx.send(42).expect("receiver still alive");
        assert_eq!(rx.await.expect("value delivered"), 42);
    });
}

#[test]
fn oneshot_send_returns_value_when_receiver_dropped() {
    let (tx, rx) = oneshot::channel();
    drop(rx);

    let err = tx.send(42).expect_err("receiver was dropped");
    assert_eq!(err, 42, "the undelivered value is returned");
}

#[test]
fn watch_receiver_cancelled_then_resumed_sees_new_value() {
    block_on(async {
        let (tx, mut rx) = watch::channel(0_u32);

        {
            let mut changed = rx.changed();
            assert_pending(&mut changed).await;
            // Cancel the blocked change future; the observed version is stale.
        }

        tx.send(1).expect("receiver still alive");
        assert!(rx.changed().await.is_ok(), "change is reported");
        assert_eq!(rx.borrow(), 1);
    });
}

#[test]
fn watch_receiver_changed_errors_when_senders_dropped() {
    block_on(async {
        let (tx, mut rx) = watch::channel(0_u32);

        {
            let mut changed = rx.changed();
            assert_pending(&mut changed).await;
        }

        drop(tx);

        assert_eq!(rx.changed().await, Err(watch::RecvError));
    });
}

#[test]
fn barrier_releases_all_waiters_when_last_arrives() {
    block_on(async {
        let barrier = sync::Barrier::new(2);

        let mut first = Box::pin(barrier.wait());
        assert_pending(&mut first).await;

        barrier.wait().await;
        guarded(first).await;
    });
}

#[test]
fn barrier_cancelled_waiter_arrival_still_counts_toward_release() {
    block_on(async {
        let barrier = sync::Barrier::new(2);

        {
            let mut first = Box::pin(barrier.wait());
            assert_pending(&mut first).await;
            // Cancel after arriving; the arrival is not reclaimed.
        }

        // One fresh arrival brings the tally to the release threshold.
        guarded(barrier.wait()).await;
    });
}
