//! Envelope encode/decode roundtrip tests

use saikuro_core::{
    capability::CapabilityToken,
    envelope::{Envelope, InvocationType, ResponseEnvelope, StreamControl},
    error::{ErrorCode, ErrorDetail},
    invocation::InvocationId,
    value::Value,
    PROTOCOL_VERSION,
};
use std::collections::BTreeMap;

// Helpers

fn roundtrip_envelope(env: &Envelope) -> Envelope {
    let bytes = env.to_msgpack().expect("serialize");
    Envelope::from_msgpack(&bytes).expect("deserialize")
}

fn roundtrip_response(resp: &ResponseEnvelope) -> ResponseEnvelope {
    let bytes = resp.to_msgpack().expect("serialize");
    ResponseEnvelope::from_msgpack(&bytes).expect("deserialize")
}

// Tests

#[test]
fn call_envelope_roundtrip() {
    let env = Envelope::call("math.add", vec![Value::Int(1), Value::Int(2)]);
    let decoded = roundtrip_envelope(&env);
    assert_eq!(decoded.version, PROTOCOL_VERSION);
    assert_eq!(decoded.invocation_type, InvocationType::Call);
    assert_eq!(decoded.target, "math.add");
    assert_eq!(decoded.args, vec![Value::Int(1), Value::Int(2)]);
    assert_eq!(decoded.id, env.id);
    assert!(decoded.capability.is_none());
    assert!(decoded.batch_items.is_none());
    assert!(decoded.stream_control.is_none());
    assert!(decoded.seq.is_none());
}

#[test]
fn cast_envelope_roundtrip() {
    let env = Envelope::cast("logger.info", vec![Value::String("hello".into())]);
    let decoded = roundtrip_envelope(&env);
    assert_eq!(decoded.invocation_type, InvocationType::Cast);
    assert_eq!(decoded.args[0], Value::String("hello".into()));
}

#[test]
fn stream_open_envelope_roundtrip() {
    let env = Envelope::stream_open("events.subscribe", vec![Value::String("topic".into())]);
    let decoded = roundtrip_envelope(&env);
    assert_eq!(decoded.invocation_type, InvocationType::Stream);
}

#[test]
fn channel_open_envelope_roundtrip() {
    let env = Envelope::channel_open("chat.session", vec![]);
    let decoded = roundtrip_envelope(&env);
    assert_eq!(decoded.invocation_type, InvocationType::Channel);
    assert!(decoded.args.is_empty());
}

#[test]
fn envelope_with_capability_roundtrip() {
    let mut env = Envelope::call("secure.op", vec![]);
    env.capability = Some(CapabilityToken::new("admin:write"));
    let decoded = roundtrip_envelope(&env);
    assert_eq!(
        decoded.capability.as_ref().map(|c| c.as_str()),
        Some("admin:write"),
    );
}

#[test]
fn envelope_with_meta_roundtrip() {
    let mut env = Envelope::call("trace.op", vec![]);
    env.meta
        .insert("trace-id".into(), Value::String("abc-123".into()));
    env.meta.insert("deadline-ms".into(), Value::Int(5000));
    let decoded = roundtrip_envelope(&env);
    assert_eq!(decoded.meta["trace-id"], Value::String("abc-123".into()));
    assert_eq!(decoded.meta["deadline-ms"], Value::Int(5000));
}

#[test]
fn batch_envelope_roundtrip() {
    let item1 = Envelope::call("math.add", vec![Value::Int(1), Value::Int(2)]);
    let item2 = Envelope::call("math.mul", vec![Value::Int(3), Value::Int(4)]);
    let mut env = Envelope::call("", vec![]);
    env.invocation_type = InvocationType::Batch;
    env.target = String::new();
    env.batch_items = Some(vec![item1, item2]);

    let decoded = roundtrip_envelope(&env);
    let items = decoded.batch_items.unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].target, "math.add");
    assert_eq!(items[1].target, "math.mul");
}

#[test]
fn stream_item_with_seq_roundtrip() {
    let id = InvocationId::new();
    let resp = ResponseEnvelope::stream_item(id, 42, Value::Float(std::f64::consts::PI));
    let decoded = roundtrip_response(&resp);
    assert!(decoded.ok);
    assert_eq!(decoded.seq, Some(42));
    assert_eq!(decoded.result, Some(Value::Float(std::f64::consts::PI)));
    assert!(decoded.stream_control.is_none());
}

#[test]
fn stream_end_sentinel_roundtrip() {
    let id = InvocationId::new();
    let resp = ResponseEnvelope::stream_end(id, 99);
    let decoded = roundtrip_response(&resp);
    assert!(decoded.ok);
    assert_eq!(decoded.seq, Some(99));
    assert_eq!(decoded.stream_control, Some(StreamControl::End));
    assert!(decoded.result.is_none());
}

#[test]
fn error_response_roundtrip() {
    let id = InvocationId::new();
    let detail = ErrorDetail::new(ErrorCode::FunctionNotFound, "no such fn");
    let resp = ResponseEnvelope::err(id, detail);
    let decoded = roundtrip_response(&resp);
    assert!(!decoded.ok);
    let err = decoded.error.unwrap();
    assert_eq!(err.code, ErrorCode::FunctionNotFound);
    assert_eq!(err.message, "no such fn");
}

#[test]
fn value_all_variants_roundtrip() {
    let cases: Vec<Value> = vec![
        Value::Null,
        Value::Bool(true),
        Value::Bool(false),
        Value::Int(-42),
        Value::UInt(u64::MAX),
        Value::Float(std::f64::consts::E),
        Value::String("hello, 世界".into()),
        Value::Bytes(vec![0x00, 0xff, 0x7e]),
        Value::Array(vec![Value::Int(1), Value::String("two".into())]),
        {
            let mut m = BTreeMap::new();
            m.insert("key".into(), Value::Bool(false));
            Value::Map(m)
        },
    ];

    for v in &cases {
        let env = Envelope::call("ns.fn", vec![v.clone()]);
        let decoded = roundtrip_envelope(&env);
        assert_eq!(
            &decoded.args[0],
            v,
            "Value variant {:?} did not roundtrip correctly",
            v.type_name()
        );
    }
}
