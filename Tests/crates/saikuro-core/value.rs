use saikuro_core::msgpack;
use saikuro_core::schema::{
    FunctionMap, FunctionSchema, NamespaceMap, NamespaceSchema, PrimitiveType, Schema,
    TypeDescriptor, TypeMap, Visibility,
};
use saikuro_event::{Value, ValueMap};

#[test]
fn check_sizes() {
    eprintln!("Value: {} bytes", std::mem::size_of::<Value>());
    eprintln!("ValueMap: {} bytes", std::mem::size_of::<ValueMap>());
}
