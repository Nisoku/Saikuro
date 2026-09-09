use crate::{shared_test, TestSuite, ToString, Vec};
use saikuro_core::envelope::Envelope;
use saikuro_core::{InvocationId, ResponseEnvelope};
use saikuro_event::{Value, ValueMap};

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "core::relay_canonical_vector_int_widths",
        relay_canonical_vector_int_widths,
    );
    shared_test!(
        suite,
        "core::relay_canonical_vector_map_order",
        relay_canonical_vector_map_order,
    );
    shared_test!(
        suite,
        "core::relay_canonical_vector_response",
        relay_canonical_vector_response,
    );
}

// Encode -> decode -> re-encode must be byte-exact.
fn assert_canonical(env: &Envelope) -> Result<(), &'static str> {
    let frame = env.to_msgpack().map_err(|_| "encode")?;
    let decoded = Envelope::from_msgpack(&frame).map_err(|_| "decode")?;
    assert_eq!(decoded.id, env.id, "id preserved");
    assert_eq!(decoded.target, env.target, "target preserved");
    let reencoded = decoded.to_msgpack().map_err(|_| "re-encode")?;
    assert!(
        frame == reencoded,
        "re-encode must equal the original frame byte-for-byte"
    );
    Ok(())
}

fn tricky_args() -> Vec<Value> {
    let mut map = ValueMap::new();
    map.insert("id".to_string(), Value::UInt(0x7fff_ffff_ffff_ffff));
    map.insert("name".to_string(), Value::String("sairiko".into()));
    vec![
        Value::UInt(7),        // positive fixint: must stay UInt, not Int
        Value::Int(-7),        // negative fixint
        Value::UInt(u64::MAX), // uint64 width
        Value::Int(i64::MIN),  // int64 width
        Value::Float(1.5),     // float64
        Value::Float(-0.0),    // sign bit preserved
        Value::Bytes(vec![0x00, 0x01, 0xfe, 0xff]),
        Value::String("héllo".into()),
        Value::Bool(true),
        Value::Map(map),
        Value::Array(vec![Value::Null, Value::Int(0)]),
    ]
}

fn relay_canonical_vector_int_widths() -> Result<(), &'static str> {
    let env = Envelope::call("probe.v2", tricky_args()).map_err(|_| "entropy")?;
    assert_canonical(&env)?;
    // Large magnitudes must survive decode with their exact width: uint64 only
    // fits UInt, int64 min only fits Int.
    let frame = env.to_msgpack().map_err(|_| "encode")?;
    let decoded = Envelope::from_msgpack(&frame).map_err(|_| "decode")?;
    let args = decoded.args;
    assert_eq!(args[2], Value::UInt(u64::MAX), "uint64 width preserved");
    assert_eq!(args[3], Value::Int(i64::MIN), "int64 width preserved");
    Ok(())
}

fn relay_canonical_vector_map_order() -> Result<(), &'static str> {
    // Inserted in deliberately non-sorted order
    let mut map = ValueMap::new();
    map.insert("zz_last".to_string(), Value::Int(1));
    map.insert("aa_first".to_string(), Value::Int(2));
    map.insert("mm_middle".to_string(), Value::Int(3));
    let env = Envelope::call("probe.order", vec![Value::Map(map)]).map_err(|_| "entropy")?;
    assert_canonical(&env)
}

fn relay_canonical_vector_response() -> Result<(), &'static str> {
    let id = InvocationId::new().map_err(|_| "entropy")?;
    let resp = ResponseEnvelope::ok(id, Value::Array(tricky_args()));
    let frame = resp.to_msgpack().map_err(|_| "encode")?;
    let decoded = ResponseEnvelope::from_msgpack(&frame).map_err(|_| "decode")?;
    assert_eq!(decoded.id, resp.id, "response id preserved");
    assert_eq!(decoded.ok, resp.ok, "response ok flag preserved");
    let result = decoded.result.as_ref().ok_or("response result")?;
    match result {
        Value::Array(items) => {
            assert_eq!(items[2], Value::UInt(u64::MAX), "uint64 width in result");
            assert_eq!(items[3], Value::Int(i64::MIN), "int64 width in result");
        }
        _ => return Err("response result must be an array"),
    }
    let reencoded = decoded.to_msgpack().map_err(|_| "re-encode")?;
    assert!(frame == reencoded, "response re-encode must be byte-exact");
    Ok(())
}
