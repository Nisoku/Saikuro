use crate::shared_test;
use crate::TestSuite;
use saikuro_exec::{JoinError, TimeoutError};

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "exec::join_error_cancelled_kind",
        join_error_cancelled_kind,
    );
    shared_test!(suite, "exec::join_error_panic_kind", join_error_panic_kind,);
    shared_test!(
        suite,
        "exec::timeout_error_display_and_eq",
        timeout_error_display_and_eq,
    );
}

fn join_error_cancelled_kind() -> Result<(), &'static str> {
    let err = JoinError::cancelled();
    crate::check_test!(err.is_cancelled(), "cancelled() must report cancelled");
    crate::check_test!(!err.is_panic(), "cancelled() must not report panic");
    crate::check_test!(
        crate::format!("{err}") == "task was cancelled",
        "cancelled join errors must display the documented wording"
    );
    Ok(())
}

fn join_error_panic_kind() -> Result<(), &'static str> {
    let err = JoinError::panic();
    crate::check_test!(err.is_panic(), "panic() must report panic");
    crate::check_test!(!err.is_cancelled(), "panic() must not report cancelled");
    crate::check_test!(
        crate::format!("{err}") == "task panicked",
        "panicked join errors must display the documented wording"
    );
    Ok(())
}

fn timeout_error_display_and_eq() -> Result<(), &'static str> {
    crate::check_test!(
        crate::format!("{TimeoutError}") == "operation timed out",
        "TimeoutError must display the documented wording"
    );
    let a = TimeoutError;
    let b = TimeoutError;
    crate::check_test!(a == b, "TimeoutError must be comparable");
    Ok(())
}
