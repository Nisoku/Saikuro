use crate::shared_test;
use crate::TestSuite;
use core::str::FromStr;
use saikuro_core::envelope::split_target;
use saikuro_core::{
    CapabilitySet, CapabilityToken, InvocationId, RegistrationToken, ResourceHandle,
    CAPABILITY_SET_CAPACITY, WILDCARD_TOKEN,
};
use saikuro_event::Value;

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "core::split_target_splits_on_last_dot",
        split_target_splits_on_last_dot
    );
    shared_test!(
        suite,
        "core::split_target_allows_dots_in_namespace",
        split_target_allows_dots_in_namespace,
    );
    shared_test!(
        suite,
        "core::split_target_rejects_leading_dot",
        split_target_rejects_leading_dot,
    );
    shared_test!(
        suite,
        "core::split_target_rejects_trailing_dot",
        split_target_rejects_trailing_dot,
    );
    shared_test!(
        suite,
        "core::split_target_rejects_missing_or_empty_target",
        split_target_rejects_missing_or_empty_target,
    );
    shared_test!(
        suite,
        "core::split_target_single_character_sides",
        split_target_single_character_sides,
    );
    shared_test!(
        suite,
        "core::resource_handle_value_roundtrip",
        resource_handle_value_roundtrip,
    );
    shared_test!(
        suite,
        "core::resource_handle_from_non_map_is_none",
        resource_handle_from_non_map_is_none,
    );
    shared_test!(
        suite,
        "core::resource_handle_negative_size_is_dropped",
        resource_handle_negative_size_is_dropped,
    );
    shared_test!(
        suite,
        "core::capability_set_at_and_over_capacity",
        capability_set_at_and_over_capacity,
    );
    shared_test!(
        suite,
        "core::capability_set_wildcard_grants_everything",
        capability_set_wildcard_grants_everything,
    );
    shared_test!(
        suite,
        "core::registration_token_monotonic",
        registration_token_monotonic,
    );
    shared_test!(
        suite,
        "core::invocation_id_string_roundtrip",
        invocation_id_string_roundtrip,
    );
}

fn split_target_splits_on_last_dot() -> Result<(), &'static str> {
    assert_eq!(split_target("ns.fn"), Some(("ns", "fn")));
    Ok(())
}

fn split_target_allows_dots_in_namespace() -> Result<(), &'static str> {
    assert_eq!(split_target("a.b.c"), Some(("a.b", "c")));
    Ok(())
}

fn split_target_rejects_leading_dot() -> Result<(), &'static str> {
    crate::check_test!(
        split_target(".fn").is_none(),
        "leading dot must be rejected"
    );
    // The split happens on the last dot, so a leading dot survives inside the
    // namespace portion rather than making the whole target invalid.
    assert_eq!(split_target(".a.b"), Some((".a", "b")));
    Ok(())
}

fn split_target_rejects_trailing_dot() -> Result<(), &'static str> {
    crate::check_test!(
        split_target("ns.").is_none(),
        "trailing dot must be rejected"
    );
    crate::check_test!(
        split_target("a.b.").is_none(),
        "trailing dot must be rejected"
    );
    Ok(())
}

fn split_target_rejects_missing_or_empty_target() -> Result<(), &'static str> {
    crate::check_test!(split_target("").is_none(), "empty target must be rejected");
    crate::check_test!(
        split_target("nodot").is_none(),
        "dot-less target must be rejected"
    );
    Ok(())
}

fn split_target_single_character_sides() -> Result<(), &'static str> {
    assert_eq!(split_target("a.b"), Some(("a", "b")));
    Ok(())
}

fn resource_handle_value_roundtrip() -> Result<(), &'static str> {
    let handle = ResourceHandle::new("file://a.txt")
        .with_mime_type("text/plain")
        .with_size(12)
        .with_uri("urn:test:1");
    let value = handle.to_value();
    let back = ResourceHandle::from_value(&value).ok_or("roundtrip must succeed")?;
    crate::check_test!(
        back.to_value() == value,
        "from_value must reproduce to_value"
    );
    Ok(())
}

fn resource_handle_from_non_map_is_none() -> Result<(), &'static str> {
    crate::check_test!(
        ResourceHandle::from_value(&Value::Int(5)).is_none(),
        "non-map value must not deserialize"
    );
    crate::check_test!(
        ResourceHandle::from_value(&Value::Null).is_none(),
        "null value must not deserialize"
    );
    Ok(())
}

fn resource_handle_negative_size_is_dropped() -> Result<(), &'static str> {
    let handle = ResourceHandle {
        id: "id".into(),
        mime_type: None,
        size: None,
        uri: None,
    };
    let map_value = ResourceHandle::new("id").to_value();
    let mut map = map_value
        .as_map()
        .ok_or("to_value must produce a map")?
        .clone();
    let _ = map.insert("size".into(), Value::Int(-3));
    let back = ResourceHandle::from_value(&Value::Map(map)).ok_or("id present")?;
    crate::check_test!(
        back == handle,
        "negative size must be dropped instead of underflowing"
    );
    Ok(())
}

fn capability_set_at_and_over_capacity() -> Result<(), &'static str> {
    let under: crate::Vec<CapabilityToken> = (0..CAPABILITY_SET_CAPACITY)
        .map(|i| CapabilityToken::new(crate::format!("cap{i}")))
        .collect();
    let set = CapabilitySet::from_tokens(under).map_err(|_| "at-capacity set must build")?;
    crate::check_test!(
        set.len() == CAPABILITY_SET_CAPACITY,
        "set must hold all tokens"
    );

    let over: crate::Vec<CapabilityToken> = (0..=CAPABILITY_SET_CAPACITY)
        .map(|i| CapabilityToken::new(crate::format!("over{i}")))
        .collect();
    crate::check_test!(
        CapabilitySet::from_tokens(over).is_err(),
        "one token over capacity must error"
    );
    Ok(())
}

fn capability_set_wildcard_grants_everything() -> Result<(), &'static str> {
    let set = CapabilitySet::all_powerful();
    crate::check_test!(
        set.grants(&CapabilityToken::new("anything.at.all")),
        "wildcard set must grant unknown tokens"
    );
    crate::check_test!(
        set.grants(&CapabilityToken::new(WILDCARD_TOKEN)),
        "wildcard set must grant the wildcard as well"
    );
    Ok(())
}

fn registration_token_monotonic() -> Result<(), &'static str> {
    let a = RegistrationToken::new();
    let b = RegistrationToken::new();
    let c = RegistrationToken::new();
    crate::check_test!(a < b, "registration tokens must be strictly increasing");
    crate::check_test!(b < c, "registration tokens must be strictly increasing");
    crate::check_test!(a != c, "registration tokens must be unique");
    Ok(())
}

fn invocation_id_string_roundtrip() -> Result<(), &'static str> {
    let text = "d05b1c3e-2f7a-4e6b-9c1d-000000000001";
    let id = InvocationId::from_str(text).map_err(|_| "canonical uuid must parse")?;
    crate::check_test!(
        crate::format!("{id}").eq(text),
        "from_str then Display must roundtrip"
    );
    let invalid = "not-a-uuid";
    crate::check_test!(
        InvocationId::from_str(invalid).is_err(),
        "malformed uuid must be rejected"
    );
    Ok(())
}
