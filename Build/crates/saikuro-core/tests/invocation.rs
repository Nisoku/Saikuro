use saikuro_core::msgpack;
use saikuro_core::InvocationId;

#[test]
fn msgpack_roundtrip_uses_binary_uuid() {
    let id = InvocationId::new();
    let encoded = msgpack::to_vec(&id).expect("encode invocation id");
    let decoded: InvocationId = msgpack::from_slice(&encoded).expect("decode invocation id");
    assert_eq!(id, decoded);
}

#[test]
fn msgpack_accepts_uuid_string_for_compatibility() {
    let uuid_text = "6f9619ff-8b86-d011-b42d-00cf4fc964ff";
    let encoded = msgpack::to_vec(&uuid_text).expect("encode uuid string payload");
    let decoded: InvocationId = msgpack::from_slice(&encoded).expect("decode uuid string payload");

    let text = decoded.to_string();
    assert_eq!(text, uuid_text);
}
