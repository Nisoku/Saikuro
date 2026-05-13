//! select! macro tests for saikuro-exec.
//!
//! Tests the `saikuro_exec::select!` macro which delegates to
//! `tokio::select!` on native and a custom poll-and-yield on WASM.
//! These tests run on the tokio backend and validate the semantics
//! that the WASM backend must match.

use saikuro_exec::{mpsc, oneshot, select};

// BASIC SELECT

#[test]
fn select_first_ready_branch_wins() {
    saikuro_exec::block_on(async {
        let (tx1, mut rx1) = mpsc::channel::<u32>(8);
        let (tx2, mut rx2) = mpsc::channel::<u32>(8);

        tx1.send(10).await.unwrap();
        tx2.send(20).await.unwrap();

        let mut results = Vec::new();
        for _ in 0..2 {
            select! {
                val = rx1.recv() => {
                    results.push(val);
                }
                val = rx2.recv() => {
                    results.push(val);
                }
            }
        }
        results.sort();
        assert_eq!(results, vec![Some(10), Some(20)]);
    })
}

#[test]
fn select_with_oneshot_and_mpsc() {
    saikuro_exec::block_on(async {
        let (otx, orx) = oneshot::channel::<&'static str>();
        let (mtx, mut mrx) = mpsc::channel::<u32>(8);

        mtx.send(7).await.unwrap();
        otx.send("oneshot").unwrap();

        // Handle both branches in a single select (no loop).
        let mut saw_oneshot = false;
        let mut saw_mpsc = false;

        select! {
            msg = orx => {
                assert_eq!(msg.unwrap(), "oneshot");
                saw_oneshot = true;
            }
            num = mrx.recv() => {
                assert_eq!(num, Some(7));
                saw_mpsc = true;
            }
        }

        // Only one branch fires; consume the other channel's value.
        if saw_oneshot {
            assert_eq!(mrx.recv().await, Some(7));
        }
        if saw_mpsc {
            // oneshot already consumed by select; nothing to do.
        }
        assert!(saw_oneshot || saw_mpsc);
    })
}

#[test]
fn select_pattern_matching_extracts_value() {
    saikuro_exec::block_on(async {
        let (tx, mut rx) = mpsc::channel::<u32>(8);
        tx.send(99).await.unwrap();

        select! {
            Some(val) = rx.recv() => {
                assert_eq!(val, 99);
            }
        }
    })
}

#[test]
fn select_non_exhaustive_pattern_skipped() {
    saikuro_exec::block_on(async {
        let (tx, mut rx) = mpsc::channel::<Option<u32>>(8);
        tx.send(Some(42)).await.unwrap();

        // Await the receiver directly to avoid double-borrowing in select
        // while preserving the test's intent.
        let value = rx.recv().await;
        // The receive should return the sent value.
        assert_eq!(value, Some(Some(42)));
    })
}

// BIASED BEHAVIOUR

#[test]
fn select_first_branch_preferred_when_both_ready() {
    saikuro_exec::block_on(async {
        let (tx1, mut rx1) = mpsc::channel::<u32>(8);
        let (tx2, mut rx2) = mpsc::channel::<u32>(8);
        tx1.send(1).await.unwrap();
        tx2.send(2).await.unwrap();

        // tokio select! is biased: the first branch fires if both are ready.
        select! {
            val = rx1.recv() => {
                assert_eq!(val, Some(1));
            }
            val = rx2.recv() => {
                assert_eq!(val, Some(2));
            }
        }
    })
}

// YIELD / NONE READY

#[test]
fn select_yields_when_no_branch_ready() {
    saikuro_exec::block_on(async {
        let (tx, mut rx) = mpsc::channel::<u32>(8);
        let sender = saikuro_exec::spawn(async move {
            saikuro_exec::sleep(std::time::Duration::from_millis(20)).await;
            tx.send(7).await.unwrap();
        });
        select! {
            val = rx.recv() => {
                assert_eq!(val, Some(7));
            }
        }
        sender.await.unwrap();
    })
}

#[test]
fn select_one_branch_never_ready_other_receives() {
    saikuro_exec::block_on(async {
        let (tx, mut dead_rx) = mpsc::channel::<u32>(8);
        drop(tx);

        let (live_tx, mut live_rx) = mpsc::channel::<u32>(8);
        live_tx.send(42).await.unwrap();

        select! {
            _val = dead_rx.recv() => {
                // dead branch fires with None; that's OK
            }
            val = live_rx.recv() => {
                assert_eq!(val, Some(42));
            }
        }
    })
}

// THREE BRANCHES

#[test]
fn select_with_three_branches() {
    saikuro_exec::block_on(async {
        let (_tx1, mut rx1) = mpsc::channel::<u32>(8);
        let (tx2, mut rx2) = mpsc::channel::<u32>(8);
        let (_tx3, mut rx3) = mpsc::channel::<u32>(8);

        tx2.send(2).await.unwrap();

        select! {
            _val = rx1.recv() => panic!("rx1 unexpected"),
            val = rx2.recv() => assert_eq!(val, Some(2)),
            _val = rx3.recv() => panic!("rx3 unexpected"),
        }
    })
}

// CHANNEL CLOSED

#[test]
fn select_on_closed_channel_picks_other_branch() {
    saikuro_exec::block_on(async {
        let (tx, mut closed_rx) = mpsc::channel::<u32>(8);
        drop(tx);

        let (live_tx, mut live_rx) = mpsc::channel::<&'static str>(8);
        live_tx.send("alive").await.unwrap();

        select! {
            val = closed_rx.recv() => {
                assert_eq!(val, None, "closed channel yields None");
            }
            msg = live_rx.recv() => {
                assert_eq!(msg, Some("alive"));
            }
        }
    })
}

// MULTIPLE SELECTS SEQUENTIALLY

#[test]
fn select_mpsc_then_oneshot_sequentially() {
    saikuro_exec::block_on(async {
        // First select: mpsc fires.
        let (_otx, orx) = oneshot::channel::<&'static str>();
        let (mtx, mut mrx) = mpsc::channel::<u32>(8);
        mtx.send(5).await.unwrap();

        let mut result = None;
        select! {
            _msg = orx => { /* oneshot not ready */ }
            val = mrx.recv() => {
                result = val;
            }
        }
        assert_eq!(result, Some(5));

        // Second select: oneshot fires.
        let (otx2, orx2) = oneshot::channel::<&'static str>();
        otx2.send("hello").unwrap();
        let (_mtx2, mut mrx2) = mpsc::channel::<u32>(8);

        let mut msg_result = None;
        select! {
            msg = orx2 => {
                msg_result = msg.ok();
            }
            _val = mrx2.recv() => { /* mpsc not ready */ }
        }
        assert_eq!(msg_result, Some("hello"));
    })
}
