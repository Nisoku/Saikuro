//! Error propagation tests

use saikuro_core::{
    error::{ErrorCode, ErrorDetail, SaikuroError},
    invocation::InvocationId,
    value::Value,
    ResponseEnvelope,
};

// SaikuroError -> ErrorDetail conversion

#[test]
fn namespace_not_found_maps_to_correct_code() {
    let err: ErrorDetail = SaikuroError::NamespaceNotFound("math".into()).into();
    assert_eq!(err.code, ErrorCode::NamespaceNotFound);
    assert!(err.message.contains("math"));
}

#[test]
fn function_not_found_maps_to_correct_code() {
    let err: ErrorDetail = SaikuroError::FunctionNotFound("math.add".into()).into();
    assert_eq!(err.code, ErrorCode::FunctionNotFound);
}

#[test]
fn invalid_arguments_maps_to_correct_code() {
    let err: ErrorDetail = SaikuroError::InvalidArguments {
        target: "math.add".into(),
        reason: "expected int".into(),
    }
    .into();
    assert_eq!(err.code, ErrorCode::InvalidArguments);
    assert!(err.message.contains("math.add"));
    assert!(err.message.contains("expected int"));
}

#[test]
fn incompatible_version_maps_correctly() {
    let err: ErrorDetail = SaikuroError::IncompatibleVersion {
        expected: 1,
        received: 99,
    }
    .into();
    assert_eq!(err.code, ErrorCode::IncompatibleVersion);
    assert!(err.message.contains("99"));
}

#[test]
fn malformed_envelope_maps_correctly() {
    let err: ErrorDetail = SaikuroError::MalformedEnvelope("missing field 'id'".into()).into();
    assert_eq!(err.code, ErrorCode::MalformedEnvelope);
}

#[test]
fn no_provider_maps_correctly() {
    let err: ErrorDetail = SaikuroError::NoProvider("events".into()).into();
    assert_eq!(err.code, ErrorCode::NoProvider);
}

#[test]
fn provider_unavailable_maps_correctly() {
    let err: ErrorDetail = SaikuroError::ProviderUnavailable("worker-1".into()).into();
    assert_eq!(err.code, ErrorCode::ProviderUnavailable);
}

#[test]
fn capability_denied_maps_correctly() {
    let err: ErrorDetail = SaikuroError::CapabilityDenied {
        target: "admin.op".into(),
        required: "admin.write".into(),
    }
    .into();
    assert_eq!(err.code, ErrorCode::CapabilityDenied);
    assert!(err.message.contains("admin.write"));
}

#[test]
fn capability_invalid_maps_correctly() {
    let err: ErrorDetail = SaikuroError::CapabilityInvalid.into();
    assert_eq!(err.code, ErrorCode::CapabilityInvalid);
}

#[test]
fn connection_lost_maps_correctly() {
    let err: ErrorDetail = SaikuroError::ConnectionLost("pipe broken".into()).into();
    assert_eq!(err.code, ErrorCode::ConnectionLost);
}

#[test]
fn message_too_large_maps_correctly() {
    let err: ErrorDetail = SaikuroError::MessageTooLarge {
        size: 10_000,
        limit: 4096,
    }
    .into();
    assert_eq!(err.code, ErrorCode::MessageTooLarge);
    assert!(err.message.contains("10000"));
}

#[test]
fn timeout_maps_correctly() {
    let err: ErrorDetail = SaikuroError::Timeout { millis: 5000 }.into();
    assert_eq!(err.code, ErrorCode::Timeout);
    assert!(err.message.contains("5000"));
}

#[test]
fn buffer_overflow_maps_correctly() {
    let err: ErrorDetail = SaikuroError::BufferOverflow.into();
    assert_eq!(err.code, ErrorCode::BufferOverflow);
}

#[test]
fn provider_error_maps_correctly() {
    let err: ErrorDetail = SaikuroError::ProviderError("db query failed".into()).into();
    assert_eq!(err.code, ErrorCode::ProviderError);
}

#[test]
fn provider_panic_maps_correctly() {
    let err: ErrorDetail = SaikuroError::ProviderPanic.into();
    assert_eq!(err.code, ErrorCode::ProviderPanic);
}

#[test]
fn stream_closed_maps_correctly() {
    let err: ErrorDetail = SaikuroError::StreamClosed.into();
    assert_eq!(err.code, ErrorCode::StreamClosed);
}

#[test]
fn channel_closed_maps_correctly() {
    let err: ErrorDetail = SaikuroError::ChannelClosed.into();
    assert_eq!(err.code, ErrorCode::ChannelClosed);
}

#[test]
fn out_of_order_maps_correctly() {
    let err: ErrorDetail = SaikuroError::OutOfOrder {
        expected: 3,
        received: 7,
    }
    .into();
    assert_eq!(err.code, ErrorCode::OutOfOrder);
    assert!(err.message.contains('3') || err.message.contains('7'));
}

#[test]
fn internal_error_maps_correctly() {
    let err: ErrorDetail = SaikuroError::Internal("unexpected state".into()).into();
    assert_eq!(err.code, ErrorCode::Internal);
}

// ErrorDetail builder

#[test]
fn error_detail_with_detail_accumulates_entries() {
    let detail = ErrorDetail::new(ErrorCode::ProviderError, "something went wrong")
        .with_detail("field", Value::String("arg_a".into()))
        .with_detail("line", Value::Int(42));

    assert_eq!(detail.details["field"], Value::String("arg_a".into()));
    assert_eq!(detail.details["line"], Value::Int(42));
}

#[test]
fn error_detail_display_includes_code_and_message() {
    let d = ErrorDetail::new(ErrorCode::Timeout, "deadline exceeded");
    let s = d.to_string();
    assert!(s.contains("Timeout"), "display: {s}");
    assert!(s.contains("deadline exceeded"), "display: {s}");
}

// MessagePack roundtrip of error responses

#[test]
fn error_response_survives_msgpack_roundtrip() {
    let id = InvocationId::new();
    let detail = ErrorDetail::new(ErrorCode::InvalidArguments, "bad types")
        .with_detail("arg", Value::String("x".into()));

    let resp = ResponseEnvelope::err(id, detail.clone());
    let bytes = resp.to_msgpack().expect("serialize");
    let decoded = ResponseEnvelope::from_msgpack(&bytes).expect("deserialize");

    assert!(!decoded.ok);
    assert_eq!(decoded.id, id);
    let err = decoded.error.expect("error should be present");
    assert_eq!(err.code, ErrorCode::InvalidArguments);
    assert_eq!(err.message, "bad types");
    assert_eq!(err.details["arg"], Value::String("x".into()));
}

#[test]
fn all_error_codes_survive_msgpack_roundtrip() {
    let codes = [
        ErrorCode::NamespaceNotFound,
        ErrorCode::FunctionNotFound,
        ErrorCode::InvalidArguments,
        ErrorCode::IncompatibleVersion,
        ErrorCode::MalformedEnvelope,
        ErrorCode::NoProvider,
        ErrorCode::ProviderUnavailable,
        ErrorCode::BatchRoutingConflict,
        ErrorCode::CapabilityDenied,
        ErrorCode::CapabilityInvalid,
        ErrorCode::ConnectionLost,
        ErrorCode::MessageTooLarge,
        ErrorCode::Timeout,
        ErrorCode::BufferOverflow,
        ErrorCode::ProviderError,
        ErrorCode::ProviderPanic,
        ErrorCode::StreamClosed,
        ErrorCode::ChannelClosed,
        ErrorCode::OutOfOrder,
        ErrorCode::Internal,
    ];

    for code in codes {
        let id = InvocationId::new();
        let detail = ErrorDetail::new(code.clone(), format!("test for {:?}", code));
        let resp = ResponseEnvelope::err(id, detail);
        let bytes = resp.to_msgpack().expect("serialize");
        let decoded = ResponseEnvelope::from_msgpack(&bytes).expect("deserialize");
        assert_eq!(
            decoded.error.unwrap().code,
            code,
            "ErrorCode {:?} did not survive roundtrip",
            code
        );
    }
}

// Router-level error propagation

#[tokio::test]
async fn provider_returns_error_response_to_caller() {
    use saikuro_core::envelope::Envelope;
    use saikuro_router::{
        provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem},
        router::InvocationRouter,
    };
    use tokio::sync::mpsc;

    let (work_tx, mut work_rx) = mpsc::channel::<ProviderWorkItem>(4);
    let handle = ProviderHandle::new("failing", vec!["fail".to_owned()], work_tx);
    let registry = ProviderRegistry::new();
    registry.register(handle);

    // Provider always returns an error.
    tokio::spawn(async move {
        while let Some(item) = work_rx.recv().await {
            if let Some(tx) = item.response_tx {
                let detail = ErrorDetail::new(ErrorCode::ProviderError, "injected failure");
                let _ = tx.send(ResponseEnvelope::err(item.envelope.id, detail));
            }
        }
    });

    let router = InvocationRouter::with_providers(registry);
    let env = Envelope::call("fail.op", vec![]);
    let resp = router.dispatch(env).await;

    assert!(!resp.ok, "call to failing provider should not be ok");
    let err = resp.error.expect("error detail");
    assert_eq!(err.code, ErrorCode::ProviderError);
    assert_eq!(err.message, "injected failure");
}
