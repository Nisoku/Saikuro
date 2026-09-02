//! Envelope encode/decode roundtrip tests

use saikuro_core::{
    capability::CapabilityToken,
    envelope::{Envelope, InvocationType, ResponseEnvelope, StreamControl},
    InvocationId, PROTOCOL_VERSION,
};
use saikuro_event::{ErrorCode, ErrorDetail, Value, ValueMap};

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
fn envelope_with_capability_roundtrip() {
    let mut env = Envelope::call("secure.op", vec![]).expect("entropy available");
    env.capability = Some(CapabilityToken::new("admin:write"));
    let decoded = roundtrip_envelope(&env);
    assert_eq!(
        decoded.capability.as_ref().map(|c| c.as_str()),
        Some("admin:write"),
    );
}
