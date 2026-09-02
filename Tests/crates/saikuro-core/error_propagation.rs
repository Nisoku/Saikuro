//! Error propagation tests

use saikuro_core::{InvocationId, ResponseEnvelope};
use saikuro_event::{ErrorCode, ErrorDetail, SaikuroError, Value};

// SaikuroError -> ErrorDetail conversion


#[test]
fn capability_invalid_maps_correctly() {
    let err: ErrorDetail = SaikuroError::CapabilityInvalid.into();
    assert_eq!(err.code, ErrorCode::CapabilityInvalid);
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
fn out_of_order_maps_correctly() {
    let err: ErrorDetail = SaikuroError::OutOfOrder {
        expected: 3,
        received: 7,
    }
    .into();
    assert_eq!(err.code, ErrorCode::OutOfOrder);
    assert!(err.message.contains('3') || err.message.contains('7'));
}


// ErrorDetail builder

#[test]
fn error_detail_with_detail_accumulates_entries() {
    let detail = ErrorDetail::new(ErrorCode::ProviderError, "something went wrong")
        .with_context("field", Value::String("arg_a".into()))
        .unwrap()
        .with_context("line", Value::Int(42))
        .unwrap();

    assert_eq!(detail.details["field"], Value::String("arg_a".into()));
    assert_eq!(detail.details["line"], Value::Int(42));
}


// MessagePack roundtrip of error responses

#[test]
fn error_response_survives_msgpack_roundtrip() {
    let id = InvocationId::new().expect("entropy available");
    let detail = ErrorDetail::new(ErrorCode::InvalidArguments, "bad types")
        .with_context("arg", Value::String("x".into()))
        .unwrap();

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


// Router-level error propagation

#[test]
fn provider_returns_error_response_to_caller() {
    saikuro_exec::block_on(async {
        use saikuro_core::envelope::Envelope;
        use saikuro_exec::mpsc;
        use saikuro_router::{
            provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem},
            router::InvocationRouter,
        };

        let (work_tx, mut work_rx) = mpsc::channel::<ProviderWorkItem>(
            saikuro_exec::ChannelCapacity::try_from(4).expect("4 is a valid channel capacity"),
        );
        let handle = ProviderHandle::new("failing", vec!["fail".to_owned()], work_tx);
        let registry = ProviderRegistry::new();
        registry.register(handle).await;

        // Provider always returns an error.
        saikuro_exec::spawn(async move {
            while let Some(item) = work_rx.recv().await {
                if let Some(tx) = item.response_tx {
                    let detail = ErrorDetail::new(ErrorCode::ProviderError, "injected failure");
                    let _ = tx.send(ResponseEnvelope::err(item.envelope.id, detail));
                }
            }
        });

        let router = InvocationRouter::with_providers(registry);
        let env = Envelope::call("fail.op", vec![]).expect("entropy available");
        let resp = router.dispatch(env).await;

        assert!(!resp.ok, "call to failing provider should not be ok");
        let err = resp.error.expect("error detail");
        assert_eq!(err.code, ErrorCode::ProviderError);
        assert_eq!(err.message, "injected failure");
    })
}
