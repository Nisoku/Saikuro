use crate::shared_test;
use crate::Box;
use crate::TestSuite;
use saikuro_core::schema::{
    FunctionMap, FunctionSchema, NamespaceMap, NamespaceSchema, PrimitiveType, Schema,
    TypeDescriptor, TypeMap, Visibility,
};
use saikuro_event::Value;

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "core::schema_round_trip_via_value",
        schema_round_trip_via_value,
    );
    shared_test!(
        suite,
        "core::array_not_confused_with_bytes",
        array_not_confused_with_bytes,
    );
    shared_test!(suite, "core::bytes_round_trip", bytes_round_trip);
    shared_test!(suite, "core::simple_map_round_trip", simple_map_round_trip);
    shared_test!(
        suite,
        "core::map_equality_ignores_insertion_order",
        map_equality_ignores_insertion_order,
    );
}

fn schema_round_trip_via_value() -> Result<(), &'static str> {
    let mut functions = FunctionMap::new();
    let _ = functions.insert(
        "add".into(),
        FunctionSchema {
            args: vec![],
            returns: TypeDescriptor::primitive(PrimitiveType::I64),
            visibility: Visibility::Public,
            capabilities: vec![],
            idempotent: false,
            doc: None,
        },
    );
    let mut namespaces = NamespaceMap::new();
    let _ = namespaces.insert(
        "math".into(),
        NamespaceSchema {
            functions: Box::new(functions),
            doc: None,
        },
    );
    let schema = Schema {
        version: 1,
        namespaces: Box::new(namespaces),
        types: Box::new(TypeMap::new()),
    };
    let val = crate::common::schema_to_value(&schema);
    let bytes = saikuro_core::to_vec(&val).map_err(|_| "serialize")?;
    let decoded: Value = saikuro_core::from_slice(&bytes).map_err(|_| "deserialize")?;
    assert_eq!(val, decoded);
    Ok(())
}

fn array_not_confused_with_bytes() -> Result<(), &'static str> {
    let arr = Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
    let bytes = saikuro_core::to_vec(&arr).map_err(|_| "serialize")?;
    assert_eq!(bytes[0], 0x93, "fixarray(3) marker");
    let decoded: Value = saikuro_core::from_slice(&bytes).map_err(|_| "deserialize")?;
    assert_eq!(decoded, arr);
    Ok(())
}

fn bytes_round_trip() -> Result<(), &'static str> {
    let b = Value::Bytes(vec![0xDE, 0xAD, 0xBE, 0xEF]);
    let bytes = saikuro_core::to_vec(&b).map_err(|_| "serialize")?;
    assert_eq!(bytes[0], 0xC4, "bin8 marker");
    let decoded: Value = saikuro_core::from_slice(&bytes).map_err(|_| "deserialize")?;
    assert_eq!(decoded, b);
    Ok(())
}

fn simple_map_round_trip() -> Result<(), &'static str> {
    let mut m = saikuro_event::ValueMap::new();
    m.insert("key".into(), Value::Int(42));
    let val = Value::Map(m);
    let bytes = saikuro_core::to_vec(&val).map_err(|_| "serialize")?;
    let decoded: Value = saikuro_core::from_slice(&bytes).map_err(|_| "deserialize")?;
    assert_eq!(decoded, val);
    Ok(())
}

fn map_equality_ignores_insertion_order() -> Result<(), &'static str> {
    let mut m1 = saikuro_event::ValueMap::new();
    m1.insert("a".into(), Value::Int(1));
    m1.insert("b".into(), Value::Int(2));
    let mut m2 = saikuro_event::ValueMap::new();
    m2.insert("b".into(), Value::Int(2));
    m2.insert("a".into(), Value::Int(1));
    assert_eq!(Value::Map(m1), Value::Map(m2));
    Ok(())
}
