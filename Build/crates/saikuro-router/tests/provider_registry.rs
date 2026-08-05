use saikuro_router::provider::{Provider, ProviderHandle, ProviderRegistry, ProviderWorkItem};

fn handle(id: &str, namespaces: &[&str]) -> ProviderHandle {
    let (sender, _receiver) = saikuro_exec::mpsc::channel::<ProviderWorkItem>(4);
    ProviderHandle::new(
        id.to_owned(),
        namespaces.iter().map(|s| s.to_string()).collect(),
        sender,
    )
}

/// Re-registering the same provider with fewer namespaces must release the
/// routes it no longer owns.
#[test]
fn register_with_fewer_namespaces_releases_dropped_routes() {
    let registry = ProviderRegistry::new();

    registry.register(handle("p", &["a", "b"]));
    assert!(registry.get("a").is_some());
    assert!(registry.get("b").is_some());

    registry.register(handle("p", &["a"]));
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

    registry.register(handle("p", &["a", "b"]));
    registry.register(handle("q", &["b"]));
    registry.register(handle("p", &["a"]));

    assert!(registry.get("a").is_some());
    let b = registry.get("b").expect("'b' is owned by q");
    assert_eq!(
        b.id(),
        "q",
        "taken-over namespace 'b' must still route to q"
    );
}
