use crate::shared_test;
use crate::TestSuite;
use crate::ToString;
use saikuro_core::{envelope::Envelope, InvocationId};

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "core::msgpack_roundtrip_uses_binary_uuid",
        msgpack_roundtrip_uses_binary_uuid,
    );
    shared_test!(
        suite,
        "core::msgpack_accepts_uuid_string",
        msgpack_accepts_uuid_string,
    );
}

fn msgpack_roundtrip_uses_binary_uuid() -> Result<(), &'static str> {
    let env = Envelope::call("a.b", vec![]).map_err(|_| "create")?;
    let bytes = env.to_msgpack().map_err(|_| "to_msgpack")?;
    let decoded = Envelope::from_msgpack(&bytes).map_err(|_| "from_msgpack")?;
    assert_eq!(env.id, decoded.id);
    Ok(())
}

fn msgpack_accepts_uuid_string() -> Result<(), &'static str> {
    let env = Envelope::call("a.b", vec![]).map_err(|_| "create")?;
    let uuid_str = env.id.to_string();
    let parsed: InvocationId = uuid_str.parse().map_err(|_| "parse uuid")?;
    assert_eq!(env.id, parsed);
    Ok(())
}
