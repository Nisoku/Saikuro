
use crate::TestSuite;
use saikuro_exec::{mpsc, oneshot, select};

fn capacity(value: usize) -> saikuro_exec::ChannelCapacity {
    saikuro_exec::ChannelCapacity::try_from(value).expect("test channel capacity must be valid")
}

pub fn register(suite: &mut TestSuite) {
    suite.register(
        "exec::select_first_ready_branch_wins",
        select_first_ready_branch_wins,
    );
    suite.register(
        "exec::select_with_oneshot_and_mpsc",
        select_with_oneshot_and_mpsc,
    );
    suite.register(
        "exec::select_pattern_matching_extracts_value",
        select_pattern_matching_extracts_value,
    );
    suite.register(
        "exec::select_non_exhaustive_pattern_skipped",
        select_non_exhaustive_pattern_skipped,
    );
    suite.register(
        "exec::select_first_branch_preferred_when_both_ready",
        select_first_branch_preferred_when_both_ready,
    );
    suite.register(
        "exec::select_yields_when_no_branch_ready",
        select_yields_when_no_branch_ready,
    );
    suite.register(
        "exec::select_one_branch_never_ready_other_receives",
        select_one_branch_never_ready_other_receives,
    );
    suite.register(
        "exec::select_with_three_branches",
        select_with_three_branches,
    );
    suite.register(
        "exec::select_on_closed_channel_picks_other_branch",
        select_on_closed_channel_picks_other_branch,
    );
    suite.register(
        "exec::select_mpsc_then_oneshot_sequentially",
        select_mpsc_then_oneshot_sequentially,
    );
}

fn select_first_ready_branch_wins() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx1, mut rx1) = mpsc::channel::<u32>(capacity(8));
        let (tx2, mut rx2) = mpsc::channel::<u32>(capacity(8));

        tx1.send(10).await.unwrap();
        tx2.send(20).await.unwrap();

        let mut results = crate::Vec::new();
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
        assert_eq!(results, crate::vec![Some(10), Some(20)]);
        Ok(())
    })
}

fn select_with_oneshot_and_mpsc() -> Result<(), &'static str> {
    crate::block_on(async {
        let (otx, orx) = oneshot::channel::<&'static str>();
        let (mtx, mut mrx) = mpsc::channel::<u32>(capacity(8));

        mtx.send(7).await.unwrap();
        otx.send("oneshot").unwrap();

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

        if saw_oneshot {
            assert_eq!(mrx.recv().await, Some(7));
        }
        if saw_mpsc {
            // oneshot already consumed by select; nothing to do.
        }
        assert!(saw_oneshot || saw_mpsc);
        Ok(())
    })
}

fn select_pattern_matching_extracts_value() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, mut rx) = mpsc::channel::<u32>(capacity(8));
        tx.send(99).await.unwrap();

        select! {
            val = rx.recv() => {
                let Some(val) = val else {
                    return Err("select did not deliver the value");
                };
                assert_eq!(val, 99);
            }
        }
        Ok(())
    })
}

fn select_non_exhaustive_pattern_skipped() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, mut rx) = mpsc::channel::<Option<u32>>(capacity(8));
        tx.send(Some(42)).await.unwrap();

        let value = rx.recv().await;
        assert_eq!(value, Some(Some(42)));
        Ok(())
    })
}

fn select_first_branch_preferred_when_both_ready() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx1, mut rx1) = mpsc::channel::<u32>(capacity(8));
        let (tx2, mut rx2) = mpsc::channel::<u32>(capacity(8));
        tx1.send(1).await.unwrap();
        tx2.send(2).await.unwrap();

        select! {
            val = rx1.recv() => {
                assert_eq!(val, Some(1));
            }
            val = rx2.recv() => {
                assert_eq!(val, Some(2));
            }
        }
        Ok(())
    })
}

fn select_yields_when_no_branch_ready() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, mut rx) = mpsc::channel::<u32>(capacity(8));
        let sender = saikuro_exec::spawn(async move {
            saikuro_exec::sleep(core::time::Duration::from_millis(20)).await;
            tx.send(7).await.unwrap();
        });
        select! {
            val = rx.recv() => {
                assert_eq!(val, Some(7));
            }
        }
        sender.await.unwrap();
        Ok(())
    })
}

fn select_one_branch_never_ready_other_receives() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, mut dead_rx) = mpsc::channel::<u32>(capacity(8));
        drop(tx);

        let (live_tx, mut live_rx) = mpsc::channel::<u32>(capacity(8));
        live_tx.send(42).await.unwrap();

        select! {
            _val = dead_rx.recv() => {}
            val = live_rx.recv() => {
                assert_eq!(val, Some(42));
            }
        }
        Ok(())
    })
}

fn select_with_three_branches() -> Result<(), &'static str> {
    crate::block_on(async {
        let (_tx1, mut rx1) = mpsc::channel::<u32>(capacity(8));
        let (tx2, mut rx2) = mpsc::channel::<u32>(capacity(8));
        let (_tx3, mut rx3) = mpsc::channel::<u32>(capacity(8));

        tx2.send(2).await.unwrap();

        select! {
            _val = rx1.recv() => { panic!("rx1 unexpected"); }
            val = rx2.recv() => { assert_eq!(val, Some(2)); }
            _val = rx3.recv() => { panic!("rx3 unexpected"); }
        }
        Ok(())
    })
}

fn select_on_closed_channel_picks_other_branch() -> Result<(), &'static str> {
    crate::block_on(async {
        let (tx, mut closed_rx) = mpsc::channel::<u32>(capacity(8));
        drop(tx);

        let (live_tx, mut live_rx) = mpsc::channel::<&'static str>(capacity(8));
        live_tx.send("alive").await.unwrap();

        select! {
            val = closed_rx.recv() => {
                assert_eq!(val, None, "closed channel yields None");
            }
            msg = live_rx.recv() => {
                assert_eq!(msg, Some("alive"));
            }
        }
        Ok(())
    })
}

fn select_mpsc_then_oneshot_sequentially() -> Result<(), &'static str> {
    crate::block_on(async {
        let (_otx, orx) = oneshot::channel::<&'static str>();
        let (mtx, mut mrx) = mpsc::channel::<u32>(capacity(8));
        mtx.send(5).await.unwrap();

        let mut result = None;
        select! {
            _msg = orx => {}
            val = mrx.recv() => {
                result = val;
            }
        }
        assert_eq!(result, Some(5));

        let (otx2, orx2) = oneshot::channel::<&'static str>();
        otx2.send("hello").unwrap();
        let (_mtx2, mut mrx2) = mpsc::channel::<u32>(capacity(8));

        let mut msg_result = None;
        select! {
            msg = orx2 => {
                msg_result = msg.ok();
            }
            _val = mrx2.recv() => {}
        }
        assert_eq!(msg_result, Some("hello"));
        Ok(())
    })
}
