//! Asyncify `block_on` tests

use crate::shared_test;
use crate::TestSuite;
use crate::Vec;
use core::time::Duration;
use saikuro_exec::{block_on, mpsc, oneshot, select, sleep, spawn, ChannelCapacity};

fn timer_leader_rearms_for_remainder() -> Result<(), &'static str> {
    block_on(async {
        for millis in [0u64, 1, 2, 3, 7, 13, 20, 21] {
            sleep(Duration::from_millis(millis)).await;
        }
        Ok(())
    })
}

fn concurrent_sleeps_all_wake() -> Result<(), &'static str> {
    block_on(async {
        let mut handles = Vec::new();
        for i in 0u64..16 {
            handles.push(spawn(async move {
                sleep(Duration::from_millis(i * 2)).await;
                i
            }));
        }
        for (expected, handle) in handles.into_iter().enumerate() {
            let actual = handle.await.map_err(|_| "sleep task reported failure")?;
            crate::check_test!(
                actual == expected as u64,
                "sleep handle returned the wrong value"
            );
        }
        Ok(())
    })
}

fn repeated_block_on_reuses_result_cell() -> Result<(), &'static str> {
    for round in 0..32u32 {
        let value = block_on(async move {
            sleep(Duration::from_millis((round % 4) as u64)).await;
            round
        });
        crate::check_test!(value == round, "round returned the wrong value");
    }
    Ok(())
}

fn select_cancels_loser() -> Result<(), &'static str> {
    block_on(async {
        let (ready_tx, ready_rx) = oneshot::channel::<u32>();
        ready_tx.send(11).unwrap();
        let (_idle_tx, mut idle_rx) = mpsc::channel::<u32>(ChannelCapacity::try_from(8).unwrap());

        let mut took_ready = false;
        let mut took_idle = false;
        let got: u32;
        select! {
            val = ready_rx => { got = val.unwrap(); took_ready = true; }
            val = idle_rx.recv() => { got = val.unwrap(); took_idle = true; }
        }
        crate::check_test!(took_ready && !took_idle, "select resolved the wrong branch");
        crate::check_test!(got == 11, "select delivered the wrong value");
        Ok(())
    })
}

fn timeout_cancels_suspended_future() -> Result<(), &'static str> {
    block_on(async {
        let result = saikuro_exec::timeout(
            Duration::from_millis(5),
            spawn(async {
                sleep(Duration::from_millis(500)).await;
            }),
        )
        .await;
        crate::check_test!(result.is_err(), "timeout did not cancel the sleeping task");
        Ok(())
    })
}

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "exec::suspend_timer_leader_rearms_for_remainder",
        timer_leader_rearms_for_remainder,
    );
    shared_test!(
        suite,
        "exec::suspend_concurrent_sleeps_all_wake",
        concurrent_sleeps_all_wake,
    );
    shared_test!(
        suite,
        "exec::suspend_repeated_block_on_reuses_result_cell",
        repeated_block_on_reuses_result_cell,
    );
    shared_test!(
        suite,
        "exec::suspend_select_cancels_loser",
        select_cancels_loser
    );
    shared_test!(
        suite,
        "exec::suspend_timeout_cancels_suspended_future",
        timeout_cancels_suspended_future,
    );
}
