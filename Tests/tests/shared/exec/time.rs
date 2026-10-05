use crate::shared_test;
use crate::TestSuite;
use core::time::Duration;
use saikuro_exec::{oneshot, sleep, spawn, timeout, yield_now};

// sleep / timeout / yield_now

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "exec::sleep_short_duration_returns",
        sleep_short_duration_returns,
    );
    shared_test!(
        suite,
        "exec::timeout_fast_future_returns_value",
        timeout_fast_future_returns_value,
    );
    shared_test!(
        suite,
        "exec::timeout_unfinished_future_reports_timeout_error",
        timeout_unfinished_future_reports_timeout_error,
    );
    shared_test!(
        suite,
        "exec::timeout_keeps_timing_after_sleep",
        timeout_keeps_timing_after_sleep,
    );
    shared_test!(
        suite,
        "exec::yield_now_returns_control_to_caller",
        yield_now_returns_control_to_caller,
    );
    shared_test!(
        suite,
        "exec::spawned_task_runs_while_callers_yield",
        spawned_task_runs_while_callers_yield,
    );
}

fn sleep_short_duration_returns() -> Result<(), &'static str> {
    crate::block_on(async {
        sleep(Duration::from_millis(10)).await;
        Ok(())
    })
}

fn timeout_fast_future_returns_value() -> Result<(), &'static str> {
    crate::block_on(async {
        let result = timeout(Duration::from_millis(100), async { 7u8 }).await;
        assert_eq!(result.map_err(|_| "fast future must not time out")?, 7);
        Ok(())
    })
}

fn timeout_unfinished_future_reports_timeout_error() -> Result<(), &'static str> {
    crate::block_on(async {
        let (_tx, gate_rx) = oneshot::channel::<u8>();
        let result = timeout(Duration::from_millis(60), gate_rx).await;
        crate::check_test!(
            result.is_err(),
            "a never-completing future must report TimeoutError"
        );
        Ok(())
    })
}

fn timeout_keeps_timing_after_sleep() -> Result<(), &'static str> {
    crate::block_on(async {
        sleep(Duration::from_millis(10)).await;
        let (_tx, gate_rx) = oneshot::channel::<u8>();
        let result = timeout(Duration::from_millis(60), gate_rx).await;
        crate::check_test!(result.is_err(), "timeout must still fire after a sleep");
        Ok(())
    })
}

fn yield_now_returns_control_to_caller() -> Result<(), &'static str> {
    crate::block_on(async {
        let mut counter = 0u8;
        for _ in 0..8 {
            yield_now().await;
            counter += 1;
        }
        crate::check_test!(
            counter == 8,
            "yield_now must return control to the caller, not block forever"
        );
        Ok(())
    })
}

fn spawned_task_runs_while_callers_yield() -> Result<(), &'static str> {
    crate::block_on(async {
        let (done_tx, done_rx) = oneshot::channel::<u8>();
        spawn(async move {
            yield_now().await;
            let _ = done_tx.send(42);
        });
        let result = timeout(Duration::from_millis(500), done_rx).await;
        let value = result.map_err(|_| "spawned task never ran")?;
        assert_eq!(value.ok(), Some(42));
        Ok(())
    })
}
