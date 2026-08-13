use saikuro_core::ResourceHandle;

#[test]
fn resource_handle_roundtrips_through_value() {
    let h = ResourceHandle::new("abc-123")
        .with_mime_type("text/plain")
        .with_size(42)
        .with_uri("saikuro://res/abc-123");

    let v = h.to_value();
    let decoded = ResourceHandle::from_value(&v).expect("decode");
    assert_eq!(decoded, h);
}

#[test]
fn resource_handle_minimal_roundtrip() {
    let h = ResourceHandle::new("xyz");
    let v = h.to_value();
    let decoded = ResourceHandle::from_value(&v).expect("decode");
    assert_eq!(decoded.id, "xyz");
    assert!(decoded.mime_type.is_none());
    assert!(decoded.size.is_none());
    assert!(decoded.uri.is_none());
}
