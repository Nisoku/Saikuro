//! `SaikuroError` code mapping, `ErrorDetail` builder, and I/O error display.

use crate::check_test;
use crate::shared_test;
use crate::TestSuite;
use crate::ToString;
use saikuro_event::{ErrorCode, ErrorDetail, IoError, IoErrorKind, SaikuroError, Value};

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "event::saikuro_error_error_code_mapping",
        saikuro_error_error_code_mapping,
    );
    shared_test!(
        suite,
        "event::saikuro_error_convenience_constructors",
        saikuro_error_convenience_constructors,
    );
    shared_test!(
        suite,
        "event::error_detail_new_and_display",
        error_detail_new_and_display,
    );
    shared_test!(
        suite,
        "event::error_detail_with_context_chain",
        error_detail_with_context_chain,
    );
    shared_test!(
        suite,
        "event::error_detail_context_over_capacity",
        error_detail_context_over_capacity,
    );
    shared_test!(
        suite,
        "event::error_detail_from_saikuro_error",
        error_detail_from_saikuro_error,
    );
    shared_test!(
        suite,
        "event::io_error_kind_and_display",
        io_error_kind_and_display,
    );
}

fn saikuro_error_error_code_mapping() -> Result<(), &'static str> {
    check_test!(
        SaikuroError::NoProvider("x".into()).error_code() == ErrorCode::NoProvider,
        "NoProvider must map directly"
    );
    check_test!(
        SaikuroError::KeyNotFound("k".into()).error_code() == ErrorCode::KeyNotFound,
        "KeyNotFound must map directly"
    );
    check_test!(
        SaikuroError::Timeout { millis: 5 }.error_code() == ErrorCode::Timeout,
        "Timeout must map directly"
    );
    check_test!(
        SaikuroError::VisibilityDenied {
            target: "t".into(),
            visibility: "private".into(),
        }
        .error_code()
            == ErrorCode::CapabilityDenied,
        "VisibilityDenied must map to CapabilityDenied"
    );
    check_test!(
        SaikuroError::SchemaCapacity.error_code() == ErrorCode::CapacityExceeded,
        "SchemaCapacity must map to CapacityExceeded"
    );
    check_test!(
        SaikuroError::Remote {
            code: "c".into(),
            message: "m".into(),
            details: None,
        }
        .error_code()
            == ErrorCode::ProviderError,
        "Remote must map to ProviderError"
    );
    check_test!(
        SaikuroError::ArgumentArity {
            expected: 2,
            received: 1,
        }
        .error_code()
            == ErrorCode::InvalidArguments,
        "ArgumentArity must map to InvalidArguments"
    );
    check_test!(
        SaikuroError::FrozenSchema("s".into()).error_code() == ErrorCode::Internal,
        "FrozenSchema must map to Internal"
    );
    check_test!(
        SaikuroError::Io(IoError {
            kind: IoErrorKind::TimedOut,
            message: None,
        })
        .error_code()
            == ErrorCode::Timeout,
        "Io timeout must map to Timeout"
    );
    check_test!(
        SaikuroError::Io(IoError {
            kind: IoErrorKind::ConnectionReset,
            message: None,
        })
        .error_code()
            == ErrorCode::ConnectionLost,
        "Io reset must map to ConnectionLost"
    );
    Ok(())
}

fn saikuro_error_convenience_constructors() -> Result<(), &'static str> {
    check_test!(
        SaikuroError::key_not_found("k").error_code() == ErrorCode::KeyNotFound,
        "key_not_found constructor"
    );
    check_test!(
        SaikuroError::namespace_not_found("ns").error_code() == ErrorCode::NamespaceNotFound,
        "namespace_not_found constructor"
    );
    check_test!(
        SaikuroError::key_already_exists("k").error_code() == ErrorCode::KeyAlreadyExists,
        "key_already_exists constructor"
    );
    check_test!(
        SaikuroError::namespace_already_exists("ns").error_code()
            == ErrorCode::NamespaceAlreadyExists,
        "namespace_already_exists constructor"
    );
    check_test!(
        SaikuroError::serialization("s").error_code() == ErrorCode::Serialization,
        "serialization constructor"
    );
    check_test!(
        SaikuroError::deserialization("d").error_code() == ErrorCode::Deserialization,
        "deserialization constructor"
    );
    check_test!(
        SaikuroError::internal("i").error_code() == ErrorCode::Internal,
        "internal constructor"
    );
    check_test!(
        SaikuroError::not_supported("n").error_code() == ErrorCode::OperationNotSupported,
        "not_supported constructor"
    );
    check_test!(
        SaikuroError::backend_not_available("b").error_code() == ErrorCode::BackendNotAvailable,
        "backend_not_available constructor"
    );
    check_test!(
        SaikuroError::quota_exceeded("q").error_code() == ErrorCode::QuotaExceeded,
        "quota_exceeded constructor"
    );
    check_test!(
        SaikuroError::remote("c", "m", None).error_code() == ErrorCode::ProviderError,
        "remote constructor"
    );
    Ok(())
}

fn error_detail_new_and_display() -> Result<(), &'static str> {
    let detail = ErrorDetail::new(ErrorCode::FunctionNotFound, "add is missing");
    check_test!(detail.code == ErrorCode::FunctionNotFound, "code preserved");
    check_test!(detail.message == "add is missing", "message preserved");
    check_test!(detail.details().is_none(), "no context by default");
    check_test!(
        detail.to_string() == "[FunctionNotFound] add is missing",
        "display must be `[code] message`"
    );
    Ok(())
}

fn error_detail_with_context_chain() -> Result<(), &'static str> {
    let detail = ErrorDetail::new(ErrorCode::InvalidArguments, "bad arg")
        .with_context("arg", Value::String("x".into()))
        .map_err(|_| "first context insert")?
        .with_context("pos", Value::Int(2))
        .map_err(|_| "second context insert")?;

    let bag = detail.details().ok_or("context bag must exist")?;
    check_test!(
        bag.get("arg").and_then(|v| v.as_str()) == Some("x"),
        "first context value must be present"
    );
    check_test!(
        bag.get("pos").and_then(|v| v.as_i64()) == Some(2),
        "second context value must be present"
    );
    Ok(())
}

fn error_detail_context_over_capacity() -> Result<(), &'static str> {
    let mut detail = ErrorDetail::new(ErrorCode::Internal, "verbose");
    for i in 0..saikuro_event::CONTEXT_CAPACITY {
        detail = detail
            .with_context(crate::format!("k{i}"), Value::Int(i as i64))
            .map_err(|_| "insertion within capacity must succeed")?;
    }
    let overflow = detail.with_context("overflow", Value::Int(0));
    check_test!(
        matches!(overflow, Err(SaikuroError::CapacityExceeded(_))),
        "insertion past capacity must error with CapacityExceeded"
    );
    Ok(())
}

fn error_detail_from_saikuro_error() -> Result<(), &'static str> {
    let source = SaikuroError::QuotaExceeded("full".into());
    let detail = ErrorDetail::from(source);
    check_test!(
        detail.code == ErrorCode::QuotaExceeded,
        "code must carry over through From"
    );
    check_test!(
        detail.to_string() == "[QuotaExceeded] quota exceeded: full",
        "message must carry the error text"
    );
    Ok(())
}

fn io_error_kind_and_display() -> Result<(), &'static str> {
    check_test!(
        IoErrorKind::TimedOut.to_string() == "timed out",
        "TimedOut must display as `timed out`"
    );
    check_test!(
        IoErrorKind::NotFound.to_string() == "not found",
        "NotFound must display as `not found`"
    );
    let bare = IoError {
        kind: IoErrorKind::NotConnected,
        message: None,
    };
    check_test!(
        bare.to_string() == "not connected",
        "bare IoError must display its kind"
    );
    let with_msg = IoError {
        kind: IoErrorKind::BrokenPipe,
        message: Some("peer hung up".into()),
    };
    check_test!(
        with_msg.to_string() == "broken pipe: peer hung up",
        "IoError must append the message"
    );
    Ok(())
}
