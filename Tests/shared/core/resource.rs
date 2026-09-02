
use crate::TestSuite;
use saikuro_core::resource::ResourceHandle;

pub fn register(suite: &mut TestSuite) {
    suite.register(
        "core::resource_handle_roundtrips_through_value",
        resource_handle_roundtrips_through_value,
    );
    suite.register(
        "core::resource_handle_minimal_roundtrip",
        resource_handle_minimal_roundtrip,
    );
}

fn resource_handle_roundtrips_through_value() -> Result<(), &'static str> {
    let handle = ResourceHandle::new("res-123").with_mime_type("file");
    let val = handle.to_value();
    let recovered = ResourceHandle::from_value(&val).ok_or("from_value")?;
    assert_eq!(handle.id, recovered.id);
    Ok(())
}

fn resource_handle_minimal_roundtrip() -> Result<(), &'static str> {
    let handle = ResourceHandle::new("x");
    let val = handle.to_value();
    let recovered = ResourceHandle::from_value(&val).ok_or("from_value")?;
    assert_eq!(recovered.id, "x");
    Ok(())
}
