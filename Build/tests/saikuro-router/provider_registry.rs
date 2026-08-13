use saikuro_core::RegistrationToken;
use saikuro_router::provider::{Provider, ProviderHandle, ProviderRegistry, ProviderWorkItem};

fn handle(id: &str, namespaces: &[&str]) -> ProviderHandle {
    handle_with_token(id, RegistrationToken::new(), namespaces)
}

fn handle_with_token(
    id: &str,
    registration_token: RegistrationToken,
    namespaces: &[&str],
) -> ProviderHandle {
    let (sender, _receiver) = saikuro_exec::mpsc::channel::<ProviderWorkItem>(
        saikuro_exec::ChannelCapacity::try_from(4).expect("4 is a valid channel capacity"),
    );
    ProviderHandle::with_registration_token(
        id.to_owned(),
        registration_token,
        namespaces.iter().map(|s| s.to_string()).collect(),
        sender,
    )
}

#[test]
fn stale_same_id_deregistration_preserves_new_registration() {
    let registry = ProviderRegistry::new();
    let old_token = RegistrationToken::new();
    let new_token = RegistrationToken::new();
    let (old_sender, _old_receiver) = saikuro_exec::mpsc::channel::<ProviderWorkItem>(
        saikuro_exec::ChannelCapacity::try_from(4).expect("4 is a valid channel capacity"),
    );
    let (new_sender, _new_receiver) = saikuro_exec::mpsc::channel::<ProviderWorkItem>(
        saikuro_exec::ChannelCapacity::try_from(4).expect("4 is a valid channel capacity"),
    );

    registry.register(ProviderHandle::with_registration_token(
        "p",
        old_token,
        vec!["service".into()],
        old_sender,
    ));
    registry.register(ProviderHandle::with_registration_token(
        "p",
        new_token,
        vec!["service".into()],
        new_sender,
    ));
    registry.deregister("p", old_token);

    let provider = registry
        .get("service")
        .expect("new provider registration remains routed");
    assert_eq!(provider.id(), "p");
    assert_eq!(provider.registration_token(), new_token);
}

/// Re-registering the same provider with fewer namespaces must release the
/// routes it no longer owns.
#[test]
fn register_with_fewer_namespaces_releases_dropped_routes() {
    let registry = ProviderRegistry::new();
    let registration_token = RegistrationToken::new();

    registry.register(handle_with_token("p", registration_token, &["a", "b"]));
    assert!(registry.get("a").is_some());
    assert!(registry.get("b").is_some());

    registry.register(handle_with_token("p", registration_token, &["a"]));
    assert!(
        registry.get("b").is_none(),
        "dropped namespace 'b' still routed after re-register"
    );
    assert!(registry.get("a").is_some());
}

/// A dropped namespace that a newer provider took over must not be released;
/// only the still-owned route is removed.
#[test]
fn register_with_fewer_namespaces_keeps_taken_over_routes() {
    let registry = ProviderRegistry::new();
    let registration_token = RegistrationToken::new();

    registry.register(handle_with_token("p", registration_token, &["a", "b"]));
    registry.register(handle("q", &["b"]));
    registry.register(handle_with_token("p", registration_token, &["a"]));

    assert!(registry.get("a").is_some());
    let b = registry.get("b").expect("'b' is owned by q");
    assert_eq!(
        b.id(),
        "q",
        "taken-over namespace 'b' must still route to q"
    );
}
