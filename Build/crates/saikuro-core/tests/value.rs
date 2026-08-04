use saikuro_core::msgpack;
use saikuro_core::schema::{
    FunctionMap, FunctionSchema, NamespaceMap, NamespaceSchema, PrimitiveType, Schema,
    TypeDescriptor, TypeMap, Visibility,
};
use saikuro_core::value::{Value, ValueMap};

/// Regression: Schema -> msgpack bytes -> Value -> msgpack bytes -> Schema must round-trip.
#[test]
fn schema_round_trip_via_value() {
    let mut functions = FunctionMap::new();
    functions
        .insert(
            "hello".to_owned(),
            FunctionSchema {
                args: vec![],
                returns: TypeDescriptor::primitive(PrimitiveType::Unit),
                visibility: Visibility::Public,
                capabilities: vec![],
                idempotent: false,
                doc: None,
            },
        )
        .expect("schema fits in FunctionMap capacity");
    let mut namespaces = NamespaceMap::new();
    namespaces
        .insert(
            "svc".to_owned(),
            NamespaceSchema {
                functions: Box::new(functions),
                doc: None,
            },
        )
        .expect("schema fits in NamespaceMap capacity");
    let schema = Schema {
        version: 1,
        namespaces: Box::new(namespaces),
        types: Box::new(TypeMap::new()),
    };

    let bytes1 = msgpack::to_vec(&schema).expect("schema to msgpack");
    let value: Value = msgpack::from_slice(&bytes1).expect("msgpack to Value");
    let bytes2 = msgpack::to_vec(&value).expect("Value to msgpack");
    let schema2: Schema = msgpack::from_slice(&bytes2).expect("msgpack to Schema");

    assert_eq!(schema2.version, 1);
    assert!(
        schema2.namespaces.contains_key("svc"),
        "namespace 'svc' not found after round-trip"
    );
}

/// Regression: Value::Array must not be confused with Value::Bytes.
#[test]
fn array_not_confused_with_bytes() {
    let original = Value::Array(vec![Value::Int(1), Value::Int(2)]);
    let bytes = msgpack::to_vec(&original).expect("serialize");
    let decoded: Value = msgpack::from_slice(&bytes).expect("deserialize");
    assert!(
        matches!(decoded, Value::Array(_)),
        "Expected Array, got: {decoded:?}"
    );
}

/// Regression: Value::Bytes must survive a round-trip as msgpack bin.
#[test]
fn bytes_round_trip() {
    let original = Value::Bytes(vec![0xde, 0xad, 0xbe, 0xef]);
    let bytes = msgpack::to_vec(&original).expect("serialize");
    let decoded: Value = msgpack::from_slice(&bytes).expect("deserialize");
    assert!(
        matches!(decoded, Value::Bytes(_)),
        "Expected Bytes, got: {decoded:?}"
    );
}

#[test]
fn check_sizes() {
    eprintln!("Value: {} bytes", std::mem::size_of::<Value>());
    eprintln!("ValueMap: {} bytes", std::mem::size_of::<ValueMap>());
}

/// Value::Map with a nested map must round-trip.
#[test]
fn simple_map_round_trip() {
    let mut inner = ValueMap::new();
    inner.insert("b".to_owned(), Value::Int(2)).expect("fits");
    let mut outer = ValueMap::new();
    outer
        .insert("a".to_owned(), Value::Map(Box::new(inner)))
        .expect("fits");
    let original = Value::Map(Box::new(outer));
    let bytes = msgpack::to_vec(&original).expect("serialize");
    let decoded: Value = msgpack::from_slice(&bytes).expect("deserialize");
    assert_eq!(original, decoded);
}
