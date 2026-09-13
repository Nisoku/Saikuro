use crate::shared_test;
use crate::TestSuite;
use saikuro_core::RegistrationToken;
use saikuro_exec::mpsc;
use saikuro_router::provider::{Provider, ProviderHandle, ProviderRegistry, ProviderWorkItem};

use crate::{ToOwned, ToString};

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "router::stale_deregister_preserves_new",
        stale_deregister_preserves_new,
    );
    shared_test!(
        suite,
        "router::register_fewer_ns_releases_routes",
        register_fewer_ns_releases_routes,
    );
    shared_test!(
        suite,
        "router::register_fewer_ns_keeps_taken_routes",
        register_fewer_ns_keeps_taken_routes,
    );
    shared_test!(
        suite,
        "router::same_id_reregister_fewer_ns_releases_routes",
        same_id_reregister_fewer_ns_releases_routes,
    );
    shared_test!(
        suite,
        "router::stale_same_id_deregister_preserves_new_token",
        stale_same_id_deregister_preserves_new_token,
    );
    shared_test!(
        suite,
        "router::same_token_reregister_keeps_taken_over_routes",
        same_token_reregister_keeps_taken_over_routes,
    );
}

fn handle_with_token(
    id: &str,
    registration_token: RegistrationToken,
    namespaces: &[&str],
) -> ProviderHandle {
    let (sender, _receiver) = mpsc::channel::<ProviderWorkItem>(saikuro_exec::ChannelCapacity::MIN);
    ProviderHandle::with_registration_token(
        id.to_owned(),
        registration_token,
        namespaces.iter().map(|s| s.to_string()).collect(),
        sender,
    )
}

fn stale_deregister_preserves_new() -> Result<(), &'static str> {
    crate::block_on(async {
        let reg = ProviderRegistry::new();
        let (tx1, _rx1) = mpsc::channel::<ProviderWorkItem>(saikuro_exec::ChannelCapacity::MIN);
        let h1 = ProviderHandle::new("old", vec!["ns1".into()], tx1);
        let token1 = h1.registration_token();
        reg.register(h1).await;

        let (tx2, _rx2) = mpsc::channel::<ProviderWorkItem>(saikuro_exec::ChannelCapacity::MIN);
        let h2 = ProviderHandle::new("new", vec!["ns1".into()], tx2);
        reg.register(h2).await;

        reg.deregister("old", token1).await;
        assert!(reg.get("ns1").await.is_some());
        Ok(())
    })
}

fn register_fewer_ns_releases_routes() -> Result<(), &'static str> {
    crate::block_on(async {
        let reg = ProviderRegistry::new();
        let (tx1, _rx1) = mpsc::channel::<ProviderWorkItem>(saikuro_exec::ChannelCapacity::MIN);
        let h1 = ProviderHandle::new("p1", vec!["a".into(), "b".into()], tx1);
        reg.register(h1).await;
        assert!(reg.get("a").await.is_some());
        assert!(reg.get("b").await.is_some());

        let (tx2, _rx2) = mpsc::channel::<ProviderWorkItem>(saikuro_exec::ChannelCapacity::MIN);
        let h2 = ProviderHandle::new("p2", vec!["a".into()], tx2);
        reg.register(h2).await;

        assert!(reg.get("a").await.is_some());
        assert!(reg.get("b").await.is_some());
        Ok(())
    })
}

fn register_fewer_ns_keeps_taken_routes() -> Result<(), &'static str> {
    crate::block_on(async {
        let reg = ProviderRegistry::new();
        let (tx1, _rx1) = mpsc::channel::<ProviderWorkItem>(saikuro_exec::ChannelCapacity::MIN);
        let h1 = ProviderHandle::new("p1", vec!["a".into(), "b".into()], tx1);
        reg.register(h1).await;

        let (tx2, _rx2) = mpsc::channel::<ProviderWorkItem>(saikuro_exec::ChannelCapacity::MIN);
        let h2 = ProviderHandle::new("p2", vec!["b".into()], tx2);
        reg.register(h2).await;

        assert!(reg.get("a").await.is_some());
        assert!(reg.get("b").await.is_some());
        Ok(())
    })
}

fn same_id_reregister_fewer_ns_releases_routes() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = ProviderRegistry::new();
        let registration_token = RegistrationToken::new();

        registry
            .register(handle_with_token("p", registration_token, &["a", "b"]))
            .await;
        assert!(registry.get("a").await.is_some());
        assert!(registry.get("b").await.is_some());

        registry
            .register(handle_with_token("p", registration_token, &["a"]))
            .await;
        assert!(
            registry.get("b").await.is_none(),
            "dropped namespace 'b' still routed after re-register"
        );
        assert!(registry.get("a").await.is_some());
        Ok(())
    })
}

fn stale_same_id_deregister_preserves_new_token() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = ProviderRegistry::new();
        let old_token = RegistrationToken::new();
        let new_token = RegistrationToken::new();

        registry
            .register(handle_with_token("p", old_token, &["service"]))
            .await;
        registry
            .register(handle_with_token("p", new_token, &["service"]))
            .await;
        registry.deregister("p", old_token).await;

        let provider = registry
            .get("service")
            .await
            .ok_or("new provider registration remains routed")?;
        assert_eq!(provider.id(), "p");
        assert_eq!(provider.registration_token(), new_token);
        Ok(())
    })
}

fn same_token_reregister_keeps_taken_over_routes() -> Result<(), &'static str> {
    crate::block_on(async {
        let registry = ProviderRegistry::new();
        let registration_token = RegistrationToken::new();

        registry
            .register(handle_with_token("p", registration_token, &["a", "b"]))
            .await;
        registry
            .register(handle_with_token("q", RegistrationToken::new(), &["b"]))
            .await;
        registry
            .register(handle_with_token("p", registration_token, &["a"]))
            .await;

        assert!(registry.get("a").await.is_some());
        let b = registry
            .get("b")
            .await
            .expect("'b' is owned by q after p drops it");
        assert_eq!(
            b.id(),
            "q",
            "taken-over namespace 'b' must still route to q"
        );
        Ok(())
    })
}
