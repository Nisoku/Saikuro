use crate::shared_test;
use crate::TestSuite;
use saikuro_exec::{oneshot, spawn};

// JoinHandle tests

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "exec::join_yields_task_output",
        join_yields_task_output
    );
    shared_test!(
        suite,
        "exec::join_wait_until_completed_returns_value",
        join_wait_until_completed_returns_value,
    );
    shared_test!(
        suite,
        "exec::abort_pending_task_then_join_cancelled",
        abort_pending_task_then_join_cancelled,
    );
    shared_test!(
        suite,
        "exec::join_many_gated_tasks_complete",
        join_many_gated_tasks_complete,
    );
}

fn join_yields_task_output() -> Result<(), &'static str> {
    crate::block_on(async {
        let handle = spawn(async { 7u32 });
        assert_eq!(handle.await.map_err(|_| "join failed")?, 7);
        Ok(())
    })
}

fn join_wait_until_completed_returns_value() -> Result<(), &'static str> {
    crate::block_on(async {
        let (gate_tx, gate_rx) = oneshot::channel::<()>();
        let handle = spawn(async move {
            let _ = gate_rx.await;
            3u8
        });
        crate::check_test!(
            !handle.is_finished(),
            "handle must stay pending while the gate is closed"
        );
        let _ = gate_tx.send(()).ok();
        assert_eq!(handle.await.map_err(|_| "join failed")?, 3);
        Ok(())
    })
}

fn abort_pending_task_then_join_cancelled() -> Result<(), &'static str> {
    crate::block_on(async {
        let (gate_tx, gate_rx) = oneshot::channel::<()>();
        let handle = spawn(async move {
            let _ = gate_rx.await;
            5u32
        });
        crate::check_test!(
            !handle.is_finished(),
            "handle must stay pending before abort"
        );
        handle.abort();
        let err = match handle.await {
            Ok(_) => return Err("join succeeded unexpectedly after abort"),
            Err(e) => e,
        };
        crate::check_test!(err.is_cancelled(), "aborted task must report cancel");
        crate::check_test!(!err.is_panic(), "aborted task must not report panic");
        // Open the gate so the base-engine task can finish and drop instead of
        // lingering in the run queue.
        let _ = gate_tx.send(()).ok();
        Ok(())
    })
}

fn join_many_gated_tasks_complete() -> Result<(), &'static str> {
    crate::block_on(async {
        const TASKS: usize = 8;
        let mut gates = crate::Vec::new();
        let mut handles = crate::Vec::new();
        for i in 0..TASKS {
            let (gate_tx, gate_rx) = oneshot::channel::<()>();
            gates.push(gate_tx);
            handles.push(spawn(async move {
                let _ = gate_rx.await;
                (i, i * 2)
            }));
        }
        for gate in gates {
            let _ = gate.send(()).ok();
        }
        let mut seen = crate::Vec::new();
        for handle in handles {
            seen.push(handle.await.map_err(|_| "join failed")?);
        }
        seen.sort();
        let expected: crate::Vec<(usize, usize)> = (0..TASKS).map(|i| (i, i * 2)).collect();
        assert_eq!(seen, expected);
        Ok(())
    })
}
