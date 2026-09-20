use crate::check_test;
use crate::shared_test;
use crate::TestSuite;
use saikuro_event::{core_to_json, json_to_core, Value, ValueMap};

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "event::value_map_insert_get_remove_crud",
        value_map_insert_get_remove_crud,
    );
    shared_test!(
        suite,
        "event::value_map_iter_yields_sorted_keys",
        value_map_iter_yields_sorted_keys,
    );
    shared_test!(
        suite,
        "event::value_map_default_is_empty",
        value_map_default_is_empty,
    );
    shared_test!(
        suite,
        "event::value_type_name_all_variants",
        value_type_name_all_variants,
    );
    shared_test!(
        suite,
        "event::value_from_conversions",
        value_from_conversions,
    );
    shared_test!(
        suite,
        "event::value_equality_ignores_map_order",
        value_equality_ignores_map_order,
    );
    shared_test!(
        suite,
        "event::value_equality_distinguishes_numeric_variants",
        value_equality_distinguishes_numeric_variants,
    );
    shared_test!(
        suite,
        "event::value_msgpack_roundtrip",
        value_msgpack_roundtrip,
    );
    shared_test!(
        suite,
        "event::json_conversions_roundtrip",
        json_conversions_roundtrip,
    );
}

fn value_map_insert_get_remove_crud() -> Result<(), &'static str> {
    let mut map = ValueMap::new();
    check_test!(map.is_empty(), "new map must be empty");

    check_test!(
        map.insert("a".into(), Value::Int(1)).is_none(),
        "new key has no old value"
    );
    check_test!(
        map.insert("a".into(), Value::Int(2)).as_ref() == Some(&Value::Int(1)),
        "overwrite must return the previous value"
    );
    check_test!(map.get("a") == Some(&Value::Int(2)), "get must read latest");
    check_test!(map.get("missing").is_none(), "absent key must be None");
    check_test!(
        map.remove("a").as_ref() == Some(&Value::Int(2)),
        "remove must return the removed value"
    );
    check_test!(map.remove("a").is_none(), "second remove must be None");
    check_test!(map.is_empty(), "map must be empty after removals");
    Ok(())
}

fn value_map_iter_yields_sorted_keys() -> Result<(), &'static str> {
    let mut map = ValueMap::new();
    map.insert("z".into(), Value::Int(26));
    map.insert("a".into(), Value::Int(1));
    map.insert("m".into(), Value::Int(13));

    let keys: crate::Vec<&str> = map.iter().map(|(k, _)| k.as_str()).collect();
    check_test!(
        keys == crate::vec!["a", "m", "z"],
        "iter must yield keys in sorted order: {keys:?}"
    );
    Ok(())
}

fn value_map_default_is_empty() -> Result<(), &'static str> {
    let map = ValueMap::default();
    check_test!(map.is_empty(), "default map must be empty");
    Ok(())
}

fn value_type_name_all_variants() -> Result<(), &'static str> {
    check_test!(Value::Null.type_name() == "null", "null type_name");
    check_test!(Value::Bool(true).type_name() == "bool", "bool type_name");
    check_test!(Value::Int(0).type_name() == "int", "int type_name");
    check_test!(Value::UInt(0).type_name() == "uint", "uint type_name");
    check_test!(Value::Float(0.0).type_name() == "float", "float type_name");
    check_test!(
        Value::String("".into()).type_name() == "string",
        "string type_name"
    );
    check_test!(
        Value::Bytes(crate::vec![]).type_name() == "bytes",
        "bytes type_name"
    );
    check_test!(
        Value::Array(crate::vec![]).type_name() == "array",
        "array type_name"
    );
    check_test!(
        Value::Map(ValueMap::new()).type_name() == "map",
        "map type_name"
    );
    Ok(())
}

fn value_from_conversions() -> Result<(), &'static str> {
    check_test!(Value::from(true) == Value::Bool(true), "bool conversion");
    check_test!(Value::from(7i32) == Value::Int(7), "i32 conversion");
    check_test!(Value::from(8i64) == Value::Int(8), "i64 conversion");
    check_test!(Value::from(9u32) == Value::UInt(9), "u32 conversion");
    check_test!(Value::from(10u64) == Value::UInt(10), "u64 conversion");
    check_test!(Value::from(2.5f32) == Value::Float(2.5), "f32 conversion");
    check_test!(Value::from(3.5f64) == Value::Float(3.5), "f64 conversion");
    check_test!(
        Value::from("s") == Value::String("s".into()),
        "&str conversion"
    );
    check_test!(
        Value::from(crate::String::from("t")) == Value::String("t".into()),
        "String conversion"
    );
    check_test!(
        Value::from(crate::vec![1u8, 2]) == Value::Bytes(crate::vec![1, 2]),
        "bytes conversion"
    );
    check_test!(
        Value::from(crate::vec![Value::Int(1), Value::Int(2)])
            == Value::Array(crate::vec![Value::Int(1), Value::Int(2)]),
        "array conversion"
    );
    check_test!(
        Value::from(Some(3u32)) == Value::UInt(3),
        "Some conversion must unwrap"
    );
    check_test!(
        Value::from(None::<u32>) == Value::Null,
        "None conversion must become Null"
    );
    Ok(())
}

fn value_equality_ignores_map_order() -> Result<(), &'static str> {
    let mut first = ValueMap::new();
    first.insert("a".into(), Value::Int(1));
    first.insert("b".into(), Value::Int(2));
    let mut second = ValueMap::new();
    second.insert("b".into(), Value::Int(2));
    second.insert("a".into(), Value::Int(1));

    check_test!(
        Value::Map(first) == Value::Map(second),
        "maps equal by content, not insertion order"
    );
    Ok(())
}

fn value_equality_distinguishes_numeric_variants() -> Result<(), &'static str> {
    check_test!(
        Value::Int(1) != Value::UInt(1),
        "Int and UInt must remain distinct"
    );
    check_test!(
        Value::Int(2) != Value::Float(2.0),
        "Int and Float must remain distinct"
    );
    check_test!(Value::Null != Value::Bool(false), "Null is not false");
    Ok(())
}

fn value_msgpack_roundtrip() -> Result<(), &'static str> {
    // Note: `Bytes` values are deliberately excluded here. The untagged
    // `Value` decoder prefers `String` for any bin payload that happens to be
    // valid UTF-8, so arbitrary byte blobs are not lossless through msgpack.
    let mut map = ValueMap::new();
    map.insert("str".into(), Value::String("hello".into()));
    map.insert("neg".into(), Value::Int(-7));
    map.insert("big".into(), Value::UInt(u64::MAX));
    map.insert("bool".into(), Value::Bool(true));
    map.insert("none".into(), Value::Null);
    let value = Value::Map(map);

    let encoded = saikuro_core::to_vec(&value).map_err(|_| "encode msgpack")?;
    let decoded: Value = saikuro_core::from_slice(&encoded).map_err(|_| "decode msgpack")?;
    check_test!(decoded == value, "complex value must roundtrip msgpack");

    let map = decoded.as_map().ok_or("decoded value must be a map")?;
    check_test!(
        map.get("big").and_then(|v| v.as_u64()) == Some(u64::MAX),
        "large uint must survive as u64"
    );
    check_test!(
        map.get("neg").and_then(|v| v.as_i64()) == Some(-7),
        "negative integer must survive"
    );

    let blob = Value::Bytes(crate::vec![0xde, 0xad]);
    let blob_encoded = saikuro_core::to_vec(&blob).map_err(|_| "encode bytes")?;
    check_test!(
        blob_encoded.first() == Some(&0xc4),
        "bytes must be transmitted as a msgpack bin8 blob: {blob_encoded:?}"
    );
    Ok(())
}

fn json_conversions_roundtrip() -> Result<(), &'static str> {
    let json: serde_json::Value =
        serde_json::from_str(r#"{"nested":{"ok":true,"items":[1,2,3]},"name":"svc","count":5}"#)
            .map_err(|_| "parse json")?;

    let core = json_to_core(json.clone());
    check_test!(core.as_map().is_some(), "object must map to a Value::Map");
    check_test!(
        core.as_map()
            .and_then(|m| m.get("name"))
            .and_then(|v| v.as_str())
            == Some("svc"),
        "string leaf must roundtrip"
    );
    check_test!(
        core.as_map()
            .and_then(|m| m.get("count"))
            .and_then(|v| v.as_i64())
            == Some(5),
        "integer leaf must roundtrip"
    );

    let back = core_to_json(core);
    check_test!(back == json, "Value -> json -> Value must be lossless");

    let simple = core_to_json(Value::Int(42));
    check_test!(
        simple == serde_json::from_str::<serde_json::Value>("42").map_err(|_| "parse")?,
        "scalar core value must map to json"
    );
    Ok(())
}
