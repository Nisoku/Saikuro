use crate::{shared_test, vec, TestSuite, ToOwned, Vec};
use saikuro_core::envelope::{classify_frame, Envelope, FrameKind, InvocationType};
use saikuro_core::{InvocationId, ResponseEnvelope};
use saikuro_event::Value;
use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

pub fn register(suite: &mut TestSuite) {
    shared_test!(suite, "core::classify_call_frame", classify_call_frame);
    shared_test!(
        suite,
        "core::classify_announce_frame",
        classify_announce_frame
    );
    shared_test!(suite, "core::classify_batch_frame", classify_batch_frame);
    shared_test!(suite, "core::classify_ok_response", classify_ok_response);
    shared_test!(suite, "core::classify_err_response", classify_err_response);
    shared_test!(
        suite,
        "core::classify_stream_item_response",
        classify_stream_item_response,
    );
    shared_test!(
        suite,
        "core::classify_garbage_and_truncated_are_nonresponse",
        classify_garbage_and_truncated_are_nonresponse,
    );
    shared_test!(
        suite,
        "core::classify_out_of_order_map_skips_values",
        classify_out_of_order_map_skips_values,
    );
    shared_test!(
        suite,
        "core::classify_skips_ext_and_bin_values",
        classify_skips_ext_and_bin_values,
    );
    shared_test!(
        suite,
        "core::classify_announce_type_after_other_keys",
        classify_announce_type_after_other_keys,
    );
}

fn classify_call_frame() -> Result<(), &'static str> {
    let env = Envelope::call("math.add", vec![Value::Int(3), Value::Int(7)])
        .map_err(|_| "entropy unavailable")?;
    let frame = env.to_msgpack().map_err(|_| "encode")?;
    assert_eq!(classify_frame(&frame), FrameKind::NonResponse);
    Ok(())
}

fn classify_announce_frame() -> Result<(), &'static str> {
    let schema_value = crate::common::schema_to_value(&saikuro_core::schema::Schema::new());
    let env = Envelope::announce(schema_value).map_err(|_| "entropy unavailable")?;
    let frame = env.to_msgpack().map_err(|_| "encode")?;
    assert_eq!(classify_frame(&frame), FrameKind::Announce);
    Ok(())
}

fn classify_batch_frame() -> Result<(), &'static str> {
    let inner = Envelope::call("a.b", vec![]).map_err(|_| "entropy unavailable")?;
    let env: Envelope = Envelope {
        version: saikuro_core::PROTOCOL_VERSION,
        invocation_type: InvocationType::Batch,
        id: InvocationId::new().map_err(|_| "entropy unavailable")?,
        target: "$saikuro.batch".to_owned(),
        args: Vec::new(),
        meta: Default::default(),
        capability: None,
        batch_items: Some(vec![inner]),
        stream_control: None,
        seq: None,
    };
    let frame = env.to_msgpack().map_err(|_| "encode")?;
    assert_eq!(classify_frame(&frame), FrameKind::NonResponse);
    Ok(())
}

fn classify_ok_response() -> Result<(), &'static str> {
    let id = InvocationId::new().map_err(|_| "entropy unavailable")?;
    let resp = ResponseEnvelope::ok(id, Value::Int(10));
    let frame = resp.to_msgpack().map_err(|_| "encode")?;
    assert_eq!(classify_frame(&frame), FrameKind::Response);
    Ok(())
}

fn classify_err_response() -> Result<(), &'static str> {
    let id = InvocationId::new().map_err(|_| "entropy unavailable")?;
    let resp = ResponseEnvelope::err(
        id,
        saikuro_event::ErrorDetail::new(
            saikuro_event::ErrorCode::FunctionNotFound,
            "boom".to_owned(),
        ),
    );
    let frame = resp.to_msgpack().map_err(|_| "encode")?;
    assert_eq!(classify_frame(&frame), FrameKind::Response);
    Ok(())
}

fn classify_stream_item_response() -> Result<(), &'static str> {
    let id = InvocationId::new().map_err(|_| "entropy unavailable")?;
    let resp = ResponseEnvelope::stream_item(id, 7, Value::String("tick".into()));
    let frame = resp.to_msgpack().map_err(|_| "encode")?;
    assert_eq!(classify_frame(&frame), FrameKind::Response);
    Ok(())
}

fn classify_garbage_and_truncated_are_nonresponse() -> Result<(), &'static str> {
    assert_eq!(classify_frame(b""), FrameKind::NonResponse);
    assert_eq!(classify_frame(b"\xff\xfe\xfd\x00"), FrameKind::NonResponse);
    assert_eq!(classify_frame(b"\x01"), FrameKind::NonResponse, "non-map");
    assert_eq!(classify_frame(b"\x80"), FrameKind::NonResponse, "empty map");

    let env = Envelope::call("a.b", vec![]).map_err(|_| "entropy unavailable")?;
    let frame = env.to_msgpack().map_err(|_| "encode")?;
    // Truncate just short of the full frame: the classifier must fall back
    // to NonResponse rather than panic or misclassify.
    assert_eq!(
        classify_frame(&frame[..frame.len() - 1]),
        FrameKind::NonResponse
    );
    Ok(())
}

// Serialize a map with a caller-controlled key order so the classifier is
// forced past non-decisive keys and must skip their values.
struct OrderedMap(Vec<(&'static str, Value)>);

impl Serialize for OrderedMap {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

fn classify_out_of_order_map_skips_values() -> Result<(), &'static str> {
    // A response whose `ok` key comes last; the parser must skip a map value
    // (nested Value::Map) and a string value first.
    let mut nested_map = saikuro_event::ValueMap::new();
    nested_map.insert("key".to_owned(), Value::String("value".into()));
    nested_map.insert("num".to_owned(), Value::Float(1.25));
    nested_map.insert("flag".to_owned(), Value::Bool(true));
    let nested: Value = Value::Map(nested_map);
    let out_of_order = OrderedMap(vec![
        ("result", nested),
        ("service", Value::String("ledger".into())),
        ("ok", Value::Bool(true)),
    ]);
    let frame = saikuro_core::msgpack::to_vec(&out_of_order).map_err(|_| "encode")?;
    assert_eq!(classify_frame(&frame), FrameKind::Response);

    // A request whose decisive `version` key comes last; the parser must skip
    // an array value and a string value first.
    let request = OrderedMap(vec![
        ("args", Value::Array(vec![Value::Int(1), Value::Int(2)])),
        ("target", Value::String("a.b".into())),
        (
            "version",
            Value::UInt(saikuro_core::PROTOCOL_VERSION as u64),
        ),
    ]);
    let frame = saikuro_core::msgpack::to_vec(&request).map_err(|_| "encode")?;
    assert_eq!(classify_frame(&frame), FrameKind::NonResponse);
    Ok(())
}

fn classify_announce_type_after_other_keys() -> Result<(), &'static str> {
    // The decisive `type` key comes after `target`; the parser must skip the
    // earlier string value, then read the `type` value and see `announce`.
    let announce = OrderedMap(vec![
        ("target", Value::String("$saikuro.announce".into())),
        ("type", Value::String("announce".into())),
        (
            "version",
            Value::UInt(saikuro_core::PROTOCOL_VERSION as u64),
        ),
    ]);
    let frame = saikuro_core::msgpack::to_vec(&announce).map_err(|_| "encode")?;
    assert_eq!(classify_frame(&frame), FrameKind::Announce);
    Ok(())
}

fn classify_skips_ext_and_bin_values() -> Result<(), &'static str> {
    // fixmap(2) | fixstr"payload" | fixext1(type=1, len=1) | fixstr"ok"
    let with_ext = b"\x82\xa7payload\xd4\x01\x55\xa2ok".to_vec();
    assert_eq!(classify_frame(&with_ext), FrameKind::Response);

    // fixmap(2) | fixstr"data" | bin8(len=3) | fixstr"type" | fixstr"call"
    let with_bin = b"\x82\xa4data\xc4\x03\x01\x02\x03\xa4type\xa4call".to_vec();
    assert_eq!(classify_frame(&with_bin), FrameKind::NonResponse);

    Ok(())
}
