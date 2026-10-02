use crate::shared_test;
use crate::TestSuite;
use core::time::Duration;
use saikuro_storage::{BackendKind, CleanupPolicy, PersistenceMode, StorageConfig};

pub fn register(suite: &mut TestSuite) {
    shared_test!(suite, "storage::config_defaults", config_defaults,);
    shared_test!(
        suite,
        "storage::config_transient_builder",
        config_transient_builder,
    );
    shared_test!(
        suite,
        "storage::config_durable_builder",
        config_durable_builder,
    );
    shared_test!(
        suite,
        "storage::config_backend_selector",
        config_backend_selector,
    );
    shared_test!(
        suite,
        "storage::config_prefix_builder",
        config_prefix_builder,
    );
    shared_test!(
        suite,
        "storage::config_cleanup_policies",
        config_cleanup_policies,
    );
    shared_test!(
        suite,
        "storage::config_sqlite_page_size_builder",
        config_sqlite_page_size_builder,
    );
    shared_test!(
        suite,
        "storage::persistence_and_backend_variants_distinct",
        persistence_and_backend_variants_distinct,
    );
}

fn config_defaults() -> Result<(), &'static str> {
    let cfg = StorageConfig::default();
    crate::check_test!(
        cfg.backend == BackendKind::InMemory,
        "the default backend must be in-memory"
    );
    crate::check_test!(
        cfg.persistence == PersistenceMode::Transient,
        "the default persistence must be transient"
    );
    crate::check_test!(
        cfg.cleanup == CleanupPolicy::Never,
        "the default cleanup must be Never"
    );
    crate::check_test!(
        cfg.namespace_prefix.is_none(),
        "there must be no default namespace prefix"
    );
    crate::check_test!(
        cfg.auto_create_namespaces,
        "namespaces must auto-create by default"
    );
    crate::check_test!(!cfg.sync_on_write, "writes must not sync by default");
    Ok(())
}

fn config_transient_builder() -> Result<(), &'static str> {
    let cfg = StorageConfig::transient();
    crate::check_test!(
        cfg.persistence == PersistenceMode::Transient,
        "transient() must select transient persistence"
    );
    crate::check_test!(
        !cfg.sync_on_write,
        "transient() must not force sync-on-write"
    );
    Ok(())
}

fn config_durable_builder() -> Result<(), &'static str> {
    let cfg = StorageConfig::durable();
    crate::check_test!(
        cfg.persistence == PersistenceMode::Durable,
        "durable() must select durable persistence"
    );
    crate::check_test!(cfg.sync_on_write, "durable() must force sync-on-write");
    Ok(())
}

fn config_backend_selector() -> Result<(), &'static str> {
    let cfg = StorageConfig::default().with_backend(BackendKind::Sqlite);
    crate::check_test!(
        cfg.backend == BackendKind::Sqlite,
        "with_backend must select the requested backend"
    );
    let cfg = StorageConfig::default().with_backend(BackendKind::Sled);
    crate::check_test!(
        cfg.backend == BackendKind::Sled,
        "with_backend must be chainable"
    );
    Ok(())
}

fn config_prefix_builder() -> Result<(), &'static str> {
    let cfg = StorageConfig::default().with_prefix("tenant-7");
    crate::check_test!(
        cfg.namespace_prefix.as_deref() == Some("tenant-7"),
        "with_prefix must set the namespace prefix"
    );
    Ok(())
}

fn config_cleanup_policies() -> Result<(), &'static str> {
    let ttl = StorageConfig::default().with_ttl(Duration::from_secs(300));
    crate::check_test!(
        ttl.cleanup == CleanupPolicy::Ttl(Duration::from_secs(300)),
        "with_ttl must set a TTL cleanup policy"
    );
    let age = CleanupPolicy::Age(Duration::from_secs(60));
    crate::check_test!(
        age == CleanupPolicy::Age(Duration::from_secs(60)),
        "Age must carry its duration"
    );
    let lru = CleanupPolicy::Lru(100);
    crate::check_test!(
        lru == CleanupPolicy::Lru(100),
        "Lru must carry its entry budget"
    );
    Ok(())
}

fn config_sqlite_page_size_builder() -> Result<(), &'static str> {
    let cfg = StorageConfig::default();
    crate::check_test!(
        cfg.sqlite_page_size == Some(1024),
        "the default sqlite page size must be 1024"
    );
    let overridden = cfg.sqlite_page_size(Some(4096));
    crate::check_test!(
        overridden.sqlite_page_size == Some(4096),
        "sqlite_page_size must override the default"
    );
    let explicit_none = overridden.sqlite_page_size(None);
    crate::check_test!(
        explicit_none.sqlite_page_size.is_none(),
        "sqlite_page_size(None) must fall back to the engine default"
    );
    Ok(())
}

fn persistence_and_backend_variants_distinct() -> Result<(), &'static str> {
    crate::check_test!(
        PersistenceMode::BestEffort != PersistenceMode::Transient
            && PersistenceMode::BestEffort != PersistenceMode::Durable,
        "every persistence mode must be distinct"
    );
    crate::check_test!(
        BackendKind::FsAccess != BackendKind::WebStorage,
        "every backend kind must be distinct"
    );
    crate::check_test!(
        CleanupPolicy::Never != CleanupPolicy::Lru(1),
        "cleanup policies must be distinct"
    );
    Ok(())
}
