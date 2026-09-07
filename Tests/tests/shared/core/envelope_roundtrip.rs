
use crate::shared_test;
use crate::TestSuite;
use saikuro_core::envelope::{Envelope, InvocationType};
use saikuro_core::ResponseEnvelope;
use saikuro_event::Value;

pub fn register(suite: &mut TestSuite) {
    shared_test!(suite, "core::call_envelope_roundtrip", call_envelope_roundtrip);
    shared_test!(suite, "core::cast_envelope_roundtrip", cast_envelope_roundtrip);
    shared_test!(suite, "core::stream_open_roundtrip", stream_open_roundtrip);
    shared_test!(suite, "core::channel_open_roundtrip", channel_open_roundtrip);
    shared_test!(suite,
        "core::envelope_with_meta_roundtrip",
        envelope_with_meta_roundtrip,
    );
    shared_test!(suite,
        "core::envelope_meta_canonical_order",
        envelope_meta_canonical_order,
    );
    shared_test!(suite, "core::batch_envelope_roundtrip", batch_envelope_roundtrip);
    shared_test!(suite,
        "core::stream_item_with_seq_roundtrip",
        stream_item_with_seq_roundtrip,
    );
    shared_test!(suite,
        "core::stream_end_sentinel_roundtrip",
        stream_end_sentinel_roundtrip,
    );
    shared_test!(suite, "core::error_response_roundtrip", error_response_roundtrip);
    shared_test!(suite,
        "core::value_all_variants_roundtrip",
        value_all_variants_roundtrip,
    );
    shared_test!(suite,
        "core::envelope_with_capability_roundtrip",
        envelope_with_capability_roundtrip,
    );
}

fn envelope_with_capability_roundtrip() -> Result<(), &'static str> {
    use saikuro_core::capability::CapabilityToken;
    let mut env = Envelope::call("secure.op", vec![]).map_err(|_| "create call")?;
    env.capability = Some(CapabilityToken::new("admin:write"));
    let bytes = env.to_msgpack().map_err(|_| "to_msgpack")?;
    let decoded = Envelope::from_msgpack(&bytes).map_err(|_| "from_msgpack")?;
    assert_eq!(
        decoded.capability.as_ref().map(|c| c.as_str()),
        Some("admin:write")
    );
    Ok(())
}

fn call_envelope_roundtrip() -> Result<(), &'static str> {
    let env = Envelope::call("math.add", vec![Value::Int(1), Value::Int(2)])
        .map_err(|_| "create call")?;
    let bytes = env.to_msgpack().map_err(|_| "to_msgpack")?;
    let decoded = Envelope::from_msgpack(&bytes).map_err(|_| "from_msgpack")?;
    assert_eq!(decoded.invocation_type, InvocationType::Call);
    assert_eq!(decoded.target, "math.add");
    Ok(())
}

fn cast_envelope_roundtrip() -> Result<(), &'static str> {
    let env = Envelope::cast("log.info", vec![Value::String("hello".into())])
        .map_err(|_| "create cast")?;
    let bytes = env.to_msgpack().map_err(|_| "to_msgpack")?;
    let decoded = Envelope::from_msgpack(&bytes).map_err(|_| "from_msgpack")?;
    assert_eq!(decoded.invocation_type, InvocationType::Cast);
    Ok(())
}

fn stream_open_roundtrip() -> Result<(), &'static str> {
    let env = Envelope::stream_open("data.stream", vec![]).map_err(|_| "create stream_open")?;
    let bytes = env.to_msgpack().map_err(|_| "to_msgpack")?;
    let decoded = Envelope::from_msgpack(&bytes).map_err(|_| "from_msgpack")?;
    assert_eq!(decoded.invocation_type, InvocationType::Stream);
    Ok(())
}

fn channel_open_roundtrip() -> Result<(), &'static str> {
    let env = Envelope::channel_open("chat.ch", vec![]).map_err(|_| "create channel_open")?;
    let bytes = env.to_msgpack().map_err(|_| "to_msgpack")?;
    let decoded = Envelope::from_msgpack(&bytes).map_err(|_| "from_msgpack")?;
    assert_eq!(decoded.invocation_type, InvocationType::Channel);
    Ok(())
}

fn envelope_with_meta_roundtrip() -> Result<(), &'static str> {
    let mut env = Envelope::call("a.b", vec![]).map_err(|_| "create")?;
    let _ = env
        .meta
        .insert("trace-id".into(), Value::String("abc123".into()));
    let bytes = env.to_msgpack().map_err(|_| "to_msgpack")?;
    let decoded = Envelope::from_msgpack(&bytes).map_err(|_| "from_msgpack")?;
    assert_eq!(
        decoded.meta.get("trace-id"),
        Some(&Value::String("abc123".into()))
    );
    Ok(())
}

fn envelope_meta_canonical_order() -> Result<(), &'static str> {
    let mut env = Envelope::call("a.b", vec![]).map_err(|_| "create")?;
    let _ = env.meta.insert("z".into(), Value::Int(1));
    let _ = env.meta.insert("a".into(), Value::Int(2));
    let _ = env.meta.insert("m".into(), Value::Int(3));
    let bytes = env.to_msgpack().map_err(|_| "msgpack")?;
    let decoded = Envelope::from_msgpack(&bytes).map_err(|_| "decode")?;
    let keys: alloc::vec::Vec<&str> = decoded.meta.iter().map(|(k, _)| k.as_str()).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    if keys != sorted {
        return Err("meta keys not in sorted order");
    }
    Ok(())
}

fn batch_envelope_roundtrip() -> Result<(), &'static str> {
    let inner1 = Envelope::call("a.b", vec![]).map_err(|_| "inner1")?;
    let inner2 = Envelope::cast("c.d", vec![Value::Bool(true)]).map_err(|_| "inner2")?;
    let mut env = Envelope::call("$saikuro.batch", vec![]).map_err(|_| "create batch")?;
    env.invocation_type = InvocationType::Batch;
    env.batch_items = Some(vec![inner1, inner2]);
    let bytes = env.to_msgpack().map_err(|_| "to_msgpack")?;
    let decoded = Envelope::from_msgpack(&bytes).map_err(|_| "from_msgpack")?;
    assert_eq!(decoded.invocation_type, InvocationType::Batch);
    Ok(())
}

fn stream_item_with_seq_roundtrip() -> Result<(), &'static str> {
    let id = saikuro_core::InvocationId::new().map_err(|_| "id")?;
    let resp = ResponseEnvelope::stream_item(id, 42, vec![Value::Int(99)].into());
    let bytes = resp.to_msgpack().map_err(|_| "to_msgpack")?;
    let decoded = ResponseEnvelope::from_msgpack(&bytes).map_err(|_| "from_msgpack")?;
    assert_eq!(decoded.seq, Some(42));
    Ok(())
}

fn stream_end_sentinel_roundtrip() -> Result<(), &'static str> {
    let id = saikuro_core::InvocationId::new().map_err(|_| "id")?;
    let resp = ResponseEnvelope::stream_end(id, 100);
    let bytes = resp.to_msgpack().map_err(|_| "to_msgpack")?;
    let decoded = ResponseEnvelope::from_msgpack(&bytes).map_err(|_| "from_msgpack")?;
    assert_eq!(decoded.seq, Some(100));
    Ok(())
}

fn error_response_roundtrip() -> Result<(), &'static str> {
    use saikuro_core::InvocationId;
    use saikuro_event::{ErrorCode, ErrorDetail};
    let id = InvocationId::new().map_err(|_| "id")?;
    let resp = ResponseEnvelope::err(id, ErrorDetail::new(ErrorCode::Timeout, "timed out"));
    let bytes = resp.to_msgpack().map_err(|_| "to_msgpack")?;
    let decoded = ResponseEnvelope::from_msgpack(&bytes).map_err(|_| "from_msgpack")?;
    assert!(!decoded.ok);
    assert_eq!(
        decoded.error.as_ref().map(|e| e.code.clone()),
        Some(ErrorCode::Timeout)
    );
    Ok(())
}

fn value_all_variants_roundtrip() -> Result<(), &'static str> {
    // Msgpack does not preserve UInt vs Int distinction for values fitting in i64.
    // Small positive integers are decoded as Int. UInt > i64::MAX roundtrips as UInt.
    // Bytes vs String distinction is lost in untagged serde + msgpack (no bin/str tag). :O
    // TODO: Consider using tagged serde for Value to preserve these distinctions.
    let values = vec![
        Value::Null,
        Value::Bool(true),
        Value::Bool(false),
        Value::Int(42),
        Value::Int(100),
        Value::UInt(u64::MAX),
        Value::Float(3.14),
        Value::String("hello".into()),
        Value::Array(vec![Value::Int(1), Value::Int(2)]),
    ];
    for val in &values {
        let bytes = saikuro_core::to_vec(val).map_err(|_| "serialize")?;
        let decoded: Value = saikuro_core::from_slice(&bytes).map_err(|_| "deserialize")?;
        assert_eq!(*val, decoded, "roundtrip failed for {:?}", val);
    }
    Ok(())
}
