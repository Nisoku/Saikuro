use crate::shared_test;
use crate::TestSuite;
use saikuro_core::{envelope::ResponseEnvelope, InvocationId};
use saikuro_event::{ErrorCode, ErrorDetail, SaikuroError, Value};

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "core::error_code_msgpack_roundtrip",
        error_code_msgpack_roundtrip,
    );
    shared_test!(
        suite,
        "core::all_error_codes_survive_msgpack",
        all_error_codes_survive_msgpack,
    );
    shared_test!(
        suite,
        "core::error_detail_display_includes_code",
        error_detail_display_includes_code,
    );
    shared_test!(
        suite,
        "core::error_detail_with_context_accumulates",
        error_detail_with_context_accumulates,
    );
    shared_test!(
        suite,
        "core::error_code_mapping_namespace_not_found",
        error_code_mapping_namespace_not_found,
    );
    shared_test!(
        suite,
        "core::error_code_mapping_function_not_found",
        error_code_mapping_function_not_found,
    );
    shared_test!(
        suite,
        "core::error_code_mapping_invalid_arguments",
        error_code_mapping_invalid_arguments,
    );
    shared_test!(
        suite,
        "core::error_code_mapping_no_provider",
        error_code_mapping_no_provider,
    );
    shared_test!(
        suite,
        "core::error_code_mapping_provider_unavailable",
        error_code_mapping_provider_unavailable,
    );
    shared_test!(
        suite,
        "core::error_code_mapping_timeout",
        error_code_mapping_timeout,
    );
    shared_test!(
        suite,
        "core::error_code_mapping_capability_denied",
        error_code_mapping_capability_denied,
    );
    shared_test!(
        suite,
        "core::error_code_mapping_message_too_large",
        error_code_mapping_message_too_large,
    );
    shared_test!(
        suite,
        "core::error_code_mapping_malformed_envelope",
        error_code_mapping_malformed_envelope,
    );
    shared_test!(
        suite,
        "core::error_code_mapping_internal_error",
        error_code_mapping_internal_error,
    );
    shared_test!(
        suite,
        "core::error_code_mapping_buffer_overflow",
        error_code_mapping_buffer_overflow,
    );
    shared_test!(
        suite,
        "core::error_code_mapping_stream_closed",
        error_code_mapping_stream_closed,
    );
    shared_test!(
        suite,
        "core::error_code_mapping_channel_closed",
        error_code_mapping_channel_closed,
    );
    shared_test!(
        suite,
        "core::error_code_mapping_connection_lost",
        error_code_mapping_connection_lost,
    );
    shared_test!(
        suite,
        "core::error_code_mapping_incompatible_version",
        error_code_mapping_incompatible_version,
    );
    shared_test!(
        suite,
        "core::saikuro_error_capability_invalid_maps",
        saikuro_error_capability_invalid_maps,
    );
    shared_test!(
        suite,
        "core::saikuro_error_provider_error_maps",
        saikuro_error_provider_error_maps,
    );
    shared_test!(
        suite,
        "core::saikuro_error_provider_panic_maps",
        saikuro_error_provider_panic_maps,
    );
    shared_test!(
        suite,
        "core::saikuro_error_out_of_order_maps",
        saikuro_error_out_of_order_maps,
    );
    shared_test!(
        suite,
        "core::error_detail_with_detail_accumulates",
        error_detail_with_detail_accumulates,
    );
    shared_test!(
        suite,
        "core::error_response_survives_msgpack_roundtrip",
        error_response_survives_msgpack_roundtrip,
    );
    shared_test!(
        suite,
        "core::provider_returns_error_response_to_caller",
        provider_returns_error_response_to_caller,
    );
}

fn error_code_msgpack_roundtrip() -> Result<(), &'static str> {
    let id = InvocationId::new().map_err(|_| "id")?;
    let resp = ResponseEnvelope::err(id, ErrorDetail::new(ErrorCode::Timeout, "oops"));
    let bytes = resp.to_msgpack().map_err(|_| "to_msgpack")?;
    let decoded = ResponseEnvelope::from_msgpack(&bytes).map_err(|_| "from_msgpack")?;
    assert!(!decoded.ok);
    assert_eq!(
        decoded.error.as_ref().map(|e| e.code.clone()),
        Some(ErrorCode::Timeout)
    );
    Ok(())
}

fn all_error_codes_survive_msgpack() -> Result<(), &'static str> {
    let codes = [
        ErrorCode::NamespaceNotFound,
        ErrorCode::FunctionNotFound,
        ErrorCode::InvalidArguments,
        ErrorCode::IncompatibleVersion,
        ErrorCode::MalformedEnvelope,
        ErrorCode::NoProvider,
        ErrorCode::ProviderUnavailable,
        ErrorCode::CapabilityDenied,
        ErrorCode::Timeout,
        ErrorCode::MessageTooLarge,
        ErrorCode::ConnectionLost,
        ErrorCode::BufferOverflow,
        ErrorCode::Internal,
        ErrorCode::StreamClosed,
        ErrorCode::ChannelClosed,
        ErrorCode::ProviderError,
        ErrorCode::ProviderPanic,
        ErrorCode::MalformedTarget,
        ErrorCode::OutOfOrder,
    ];
    for code in &codes {
        let id = InvocationId::new().map_err(|_| "id")?;
        let resp = ResponseEnvelope::err(id, ErrorDetail::new(code.clone(), "test"));
        let bytes = resp.to_msgpack().map_err(|_| "to_msgpack")?;
        let decoded = ResponseEnvelope::from_msgpack(&bytes).map_err(|_| "from_msgpack")?;
        assert_eq!(
            decoded.error.as_ref().map(|e| e.code.clone()),
            Some(code.clone()),
            "roundtrip failed for {:?}",
            code
        );
    }
    Ok(())
}

fn error_detail_display_includes_code() -> Result<(), &'static str> {
    let detail = ErrorDetail::new(ErrorCode::Timeout, "timed out");
    let s = alloc::format!("{}", detail);
    assert!(s.contains("Timeout"));
    Ok(())
}

fn error_detail_with_context_accumulates() -> Result<(), &'static str> {
    let detail = ErrorDetail::new(ErrorCode::Internal, "initial");
    let detail = detail
        .with_context("key", "extra info")
        .map_err(|_| "with_context")?;
    let s = alloc::format!("{}", detail);
    assert!(s.contains("initial"));
    assert!(detail.details().expect("context present").get("key").is_some());
    Ok(())
}

fn error_code_mapping_namespace_not_found() -> Result<(), &'static str> {
    let err = SaikuroError::NamespaceNotFound("ns".into());
    assert_eq!(err.error_code(), ErrorCode::NamespaceNotFound);
    Ok(())
}

fn error_code_mapping_function_not_found() -> Result<(), &'static str> {
    let err = SaikuroError::FunctionNotFound("fn".into());
    assert_eq!(err.error_code(), ErrorCode::FunctionNotFound);
    Ok(())
}

fn error_code_mapping_invalid_arguments() -> Result<(), &'static str> {
    let err = SaikuroError::InvalidArguments { target: "ns.fn".into(), reason: "bad arg".into() };
    assert_eq!(err.error_code(), ErrorCode::InvalidArguments);
    Ok(())
}

fn error_code_mapping_no_provider() -> Result<(), &'static str> {
    let err = SaikuroError::NoProvider("ns".into());
    assert_eq!(err.error_code(), ErrorCode::NoProvider);
    Ok(())
}

fn error_code_mapping_provider_unavailable() -> Result<(), &'static str> {
let err = SaikuroError::ProviderUnavailable("ns".into());
    assert_eq!(err.error_code(), ErrorCode::ProviderUnavailable);
    Ok(())
}

fn error_code_mapping_timeout() -> Result<(), &'static str> {
    let err = SaikuroError::Timeout { millis: 1000 };
    assert_eq!(err.error_code(), ErrorCode::Timeout);
    Ok(())
}

fn error_code_mapping_capability_denied() -> Result<(), &'static str> {
    let err = SaikuroError::CapabilityDenied { target: "ns.fn".into(), required: "cap".into() };
    assert_eq!(err.error_code(), ErrorCode::CapabilityDenied);
    Ok(())
}

fn error_code_mapping_message_too_large() -> Result<(), &'static str> {
    let err = SaikuroError::MessageTooLarge { size: 1024, limit: 512 };
    assert_eq!(err.error_code(), ErrorCode::MessageTooLarge);
    Ok(())
}

fn error_code_mapping_malformed_envelope() -> Result<(), &'static str> {
    let err = SaikuroError::MalformedEnvelope("bad".into());
    assert_eq!(err.error_code(), ErrorCode::MalformedEnvelope);
    Ok(())
}

fn error_code_mapping_internal_error() -> Result<(), &'static str> {
    let err = SaikuroError::Internal("boom".into());
    assert_eq!(err.error_code(), ErrorCode::Internal);
    Ok(())
}

fn error_code_mapping_buffer_overflow() -> Result<(), &'static str> {
    let err = SaikuroError::BufferOverflow;
    assert_eq!(err.error_code(), ErrorCode::BufferOverflow);
    Ok(())
}

fn error_code_mapping_stream_closed() -> Result<(), &'static str> {
    let err = SaikuroError::StreamClosed;
    assert_eq!(err.error_code(), ErrorCode::StreamClosed);
    Ok(())
}

fn error_code_mapping_channel_closed() -> Result<(), &'static str> {
    let err = SaikuroError::ChannelClosed;
    assert_eq!(err.error_code(), ErrorCode::ChannelClosed);
    Ok(())
}

fn error_code_mapping_connection_lost() -> Result<(), &'static str> {
    let err = SaikuroError::ConnectionLost("reset".into());
    assert_eq!(err.error_code(), ErrorCode::ConnectionLost);
    Ok(())
}

fn error_code_mapping_incompatible_version() -> Result<(), &'static str> {
let err = SaikuroError::IncompatibleVersion { expected: 1, received: 2 };
    assert_eq!(err.error_code(), ErrorCode::IncompatibleVersion);
    Ok(())
}

fn saikuro_error_capability_invalid_maps() -> Result<(), &'static str> {
    let err: ErrorDetail = SaikuroError::CapabilityInvalid.into();
    assert_eq!(err.code, ErrorCode::CapabilityInvalid);
    Ok(())
}

fn saikuro_error_provider_error_maps() -> Result<(), &'static str> {
    let err: ErrorDetail = SaikuroError::ProviderError("db query failed".into()).into();
    assert_eq!(err.code, ErrorCode::ProviderError);
    Ok(())
}

fn saikuro_error_provider_panic_maps() -> Result<(), &'static str> {
    let err: ErrorDetail = SaikuroError::ProviderPanic.into();
    assert_eq!(err.code, ErrorCode::ProviderPanic);
    Ok(())
}

fn saikuro_error_out_of_order_maps() -> Result<(), &'static str> {
    let err: ErrorDetail = SaikuroError::OutOfOrder {
        expected: 3,
        received: 7,
    }
    .into();
    assert_eq!(err.code, ErrorCode::OutOfOrder);
    assert!(err.message.contains('3') || err.message.contains('7'));
    Ok(())
}

fn error_detail_with_detail_accumulates() -> Result<(), &'static str> {
    let detail = ErrorDetail::new(ErrorCode::ProviderError, "something went wrong")
        .with_context("field", Value::String("arg_a".into()))
        .map_err(|_| "with_context")?
        .with_context("line", Value::Int(42))
        .map_err(|_| "with_context")?;

    let details = detail.details().expect("context present");
    assert_eq!(details["field"], Value::String("arg_a".into()));
    assert_eq!(details["line"], Value::Int(42));
    Ok(())
}

fn error_response_survives_msgpack_roundtrip() -> Result<(), &'static str> {
    let id = InvocationId::new().map_err(|_| "id")?;
    let detail = ErrorDetail::new(ErrorCode::InvalidArguments, "bad types")
        .with_context("arg", Value::String("x".into()))
        .map_err(|_| "with_context")?;

    let resp = ResponseEnvelope::err(id, detail.clone());
    let bytes = resp.to_msgpack().map_err(|_| "serialize")?;
    let decoded = ResponseEnvelope::from_msgpack(&bytes).map_err(|_| "deserialize")?;

    assert!(!decoded.ok);
    assert_eq!(decoded.id, id);
    let err = decoded.error.as_ref().expect("error should be present");
    assert_eq!(err.code, ErrorCode::InvalidArguments);
    assert_eq!(err.message, "bad types");
    assert_eq!(err.details().expect("context present")["arg"], Value::String("x".into()));
    Ok(())
}

fn provider_returns_error_response_to_caller() -> Result<(), &'static str> {
    crate::block_on(async {
        use saikuro_core::envelope::Envelope;
        use saikuro_exec::mpsc;
        use saikuro_router::{
            provider::{ProviderHandle, ProviderRegistry, ProviderWorkItem},
            router::InvocationRouter,
        };

        let (work_tx, mut work_rx) = mpsc::channel::<ProviderWorkItem>(crate::common::capacity(4));
        let handle = ProviderHandle::new("failing", vec!["fail".into()], work_tx);
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
        let env = Envelope::call("fail.op", vec![]).map_err(|_| "create")?;
        let resp = router.dispatch(env).await;

        assert!(!resp.ok, "call to failing provider should not be ok");
        let err = resp.error.as_ref().expect("error detail");
        assert_eq!(err.code, ErrorCode::ProviderError);
        assert_eq!(err.message, "injected failure");
        Ok(())
    })
}
