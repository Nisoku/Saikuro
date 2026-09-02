//! In-memory storage backend tests.

use saikuro_tests::common;
use saikuro_tests::TestSuite;
use bytes::Bytes;
use saikuro_storage::{InMemoryStorage, KeyValueBackend, StorageBackend, StorageConfig};

pub fn register(suite: &mut TestSuite) {
    suite.register("storage::inmemory_new_creates_empty_store", new_creates_empty_store);
    suite.register("storage::inmemory_with_config_applies_config", with_config_applies_config);
    suite.register("storage::inmemory_put_and_get_roundtrip", put_and_get_roundtrip);
    suite.register("storage::inmemory_get_missing_key_returns_none", get_missing_key_returns_none);
    suite.register("storage::inmemory_exists_true_for_existing", exists_returns_true_for_existing_key);
    suite.register("storage::inmemory_exists_false_for_missing", exists_returns_false_for_missing_key);
    suite.register("storage::inmemory_exists_errors_on_missing_namespace", exists_errors_on_missing_namespace);
    suite.register("storage::inmemory_put_overwrites_existing", put_overwrites_existing);
    suite.register("storage::inmemory_put_and_get_binary_data", put_and_get_binary_data);
    suite.register("storage::inmemory_delete_removes_key", delete_removes_key);
    suite.register("storage::inmemory_delete_missing_key_ok", delete_missing_key_does_not_error);
    suite.register("storage::inmemory_list_keys_returns_all", list_keys_returns_all_keys);
    suite.register("storage::inmemory_list_keys_empty_namespace", list_keys_empty_namespace);
    suite.register("storage::inmemory_list_keys_isolates_namespaces", list_keys_isolates_namespaces);
    suite.register("storage::inmemory_list_namespaces_returns_all", list_namespaces_returns_all);
    suite.register("storage::inmemory_list_namespaces_empty", list_namespaces_empty_when_no_data);
    suite.register("storage::inmemory_create_namespace_then_crud", create_namespace_then_put_and_get);
    suite.register("storage::inmemory_create_existing_namespace_errors", create_existing_namespace_errors);
    suite.register("storage::inmemory_delete_namespace_removes_keys", delete_namespace_removes_all_keys);
    suite.register("storage::inmemory_delete_missing_namespace_ok", delete_nonexistent_namespace_does_not_error);
    suite.register("storage::inmemory_clear_namespace_empties_keys", clear_namespace_empties_keys);
    suite.register("storage::inmemory_clear_namespace_preserves_ns", clear_namespace_preserves_namespace);
    suite.register("storage::inmemory_put_auto_creates_namespace", put_auto_creates_namespace_by_default);
    suite.register("storage::inmemory_put_fails_without_auto_create", put_fails_when_auto_create_disabled);
    suite.register("storage::inmemory_get_fails_without_auto_create", get_fails_on_missing_namespace_without_auto_create);
    suite.register("storage::inmemory_prefix_isolates_storage", namespace_prefix_isolates_storage);
    suite.register("storage::inmemory_prefix_list_namespaces_stripped", namespace_prefix_list_namespaces_is_stripped);
    suite.register("storage::inmemory_supports_files_false", supports_files_is_false);
    suite.register("storage::inmemory_as_file_backend_none", as_file_backend_is_none);
}

async fn storage_with(cfg: StorageConfig) -> InMemoryStorage {
    InMemoryStorage::with_config(cfg, common::null_log()).await
}

fn new_creates_empty_store() -> Result<(), &'static str> {
    let s = InMemoryStorage::new();
    assert_eq!(s.config(), &StorageConfig::default());
    Ok(())
}

fn with_config_applies_config() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let cfg = StorageConfig::durable().with_prefix("test");
        let s = storage_with(cfg.clone()).await;
        assert_eq!(s.config(), &cfg);
        Ok(())
    })
}

fn put_and_get_roundtrip() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = InMemoryStorage::new();
        s.put("ns", "k", Bytes::from("hello")).await.map_err(|_| "put")?;
        let v = s.get("ns", "k").await.map_err(|_| "get")?;
        assert_eq!(v, Some(Bytes::from("hello")));
        Ok(())
    })
}

fn get_missing_key_returns_none() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = InMemoryStorage::new();
        let v = s.get("ns", "missing").await.map_err(|_| "get")?;
        assert_eq!(v, None);
        Ok(())
    })
}

fn exists_returns_true_for_existing_key() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = InMemoryStorage::new();
        s.put("ns", "k", Bytes::from("v")).await.map_err(|_| "put")?;
        assert!(s.exists("ns", "k").await.map_err(|_| "exists")?);
        Ok(())
    })
}

fn exists_returns_false_for_missing_key() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = InMemoryStorage::new();
        assert!(!s.exists("ns", "missing").await.map_err(|_| "exists")?);
        Ok(())
    })
}

fn exists_errors_on_missing_namespace() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let cfg = StorageConfig {
            auto_create_namespaces: false,
            namespace_prefix: Some("x".into()),
            ..Default::default()
        };
        let s = storage_with(cfg).await;
        let r = s.exists("nonexistent", "k").await;
        assert!(r.is_err());
        Ok(())
    })
}

fn put_overwrites_existing() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = InMemoryStorage::new();
        s.put("ns", "k", Bytes::from("v1")).await.map_err(|_| "put1")?;
        s.put("ns", "k", Bytes::from("v2")).await.map_err(|_| "put2")?;
        let v = s.get("ns", "k").await.map_err(|_| "get")?;
        assert_eq!(v, Some(Bytes::from("v2")));
        Ok(())
    })
}

fn put_and_get_binary_data() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = InMemoryStorage::new();
        let data: Bytes = (0..=255).collect();
        s.put("ns", "bin", data.clone()).await.map_err(|_| "put")?;
        let v = s.get("ns", "bin").await.map_err(|_| "get")?;
        assert_eq!(v, Some(data));
        Ok(())
    })
}

fn delete_removes_key() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = InMemoryStorage::new();
        s.put("ns", "k", Bytes::from("v")).await.map_err(|_| "put")?;
        s.delete("ns", "k").await.map_err(|_| "delete")?;
        assert!(!s.exists("ns", "k").await.map_err(|_| "exists")?);
        Ok(())
    })
}

fn delete_missing_key_does_not_error() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = InMemoryStorage::new();
        s.delete("ns", "missing").await.map_err(|_| "delete")?;
        Ok(())
    })
}

fn list_keys_returns_all_keys() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = InMemoryStorage::new();
        s.put("ns", "a", Bytes::from("1")).await.map_err(|_| "put a")?;
        s.put("ns", "b", Bytes::from("2")).await.map_err(|_| "put b")?;
        let mut keys = s.list_keys("ns").await.map_err(|_| "list_keys")?;
        keys.sort();
        assert_eq!(
            keys,
            vec![saikuro_tests::String::from("a"), saikuro_tests::String::from("b")]
        );
        Ok(())
    })
}

fn list_keys_empty_namespace() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = InMemoryStorage::new();
        let keys = s.list_keys("ns").await.map_err(|_| "list_keys")?;
        assert!(keys.is_empty());
        Ok(())
    })
}

fn list_keys_isolates_namespaces() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = InMemoryStorage::new();
        s.put("ns1", "k", Bytes::from("v")).await.map_err(|_| "put1")?;
        s.put("ns2", "k", Bytes::from("v")).await.map_err(|_| "put2")?;
        let keys1 = s.list_keys("ns1").await.map_err(|_| "list1")?;
        assert_eq!(keys1, vec![saikuro_tests::String::from("k")]);
        let keys2 = s.list_keys("ns2").await.map_err(|_| "list2")?;
        assert_eq!(keys2, vec![saikuro_tests::String::from("k")]);
        Ok(())
    })
}

fn list_namespaces_returns_all() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = InMemoryStorage::new();
        s.put("ns1", "a", Bytes::from("1")).await.map_err(|_| "put1")?;
        s.put("ns2", "b", Bytes::from("2")).await.map_err(|_| "put2")?;
        let mut nss = s.list_namespaces().await.map_err(|_| "list_namespaces")?;
        nss.sort();
        assert_eq!(
            nss,
            vec![saikuro_tests::String::from("ns1"), saikuro_tests::String::from("ns2")]
        );
        Ok(())
    })
}

fn list_namespaces_empty_when_no_data() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = InMemoryStorage::new();
        let nss = s.list_namespaces().await.map_err(|_| "list_namespaces")?;
        assert!(nss.is_empty());
        Ok(())
    })
}

fn create_namespace_then_put_and_get() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = InMemoryStorage::new();
        s.create_namespace("explicit")
            .await
            .map_err(|_| "create_namespace")?;
        s.put("explicit", "k", Bytes::from("v"))
            .await
            .map_err(|_| "put")?;
        let v = s.get("explicit", "k").await.map_err(|_| "get")?;
        assert_eq!(v, Some(Bytes::from("v")));
        Ok(())
    })
}

fn create_existing_namespace_errors() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = InMemoryStorage::new();
        s.create_namespace("ns").await.map_err(|_| "create")?;
        let r = s.create_namespace("ns").await;
        assert!(r.is_err());
        Ok(())
    })
}

fn delete_namespace_removes_all_keys() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = InMemoryStorage::new();
        s.put("ns", "k", Bytes::from("v")).await.map_err(|_| "put")?;
        s.delete_namespace("ns").await.map_err(|_| "delete_namespace")?;
        assert_eq!(s.get("ns", "k").await.map_err(|_| "get")?, None);
        Ok(())
    })
}

fn delete_nonexistent_namespace_does_not_error() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = InMemoryStorage::new();
        s.delete_namespace("nowhere")
            .await
            .map_err(|_| "delete_namespace")?;
        Ok(())
    })
}

fn clear_namespace_empties_keys() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = InMemoryStorage::new();
        s.put("ns", "k", Bytes::from("v")).await.map_err(|_| "put")?;
        s.clear_namespace("ns").await.map_err(|_| "clear")?;
        assert!(!s.exists("ns", "k").await.map_err(|_| "exists")?);
        Ok(())
    })
}

fn clear_namespace_preserves_namespace() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = InMemoryStorage::new();
        s.put("ns", "k", Bytes::from("v")).await.map_err(|_| "put1")?;
        s.clear_namespace("ns").await.map_err(|_| "clear")?;
        s.put("ns", "k2", Bytes::from("v")).await.map_err(|_| "put2")?;
        assert!(s.exists("ns", "k2").await.map_err(|_| "exists")?);
        Ok(())
    })
}

fn put_auto_creates_namespace_by_default() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = InMemoryStorage::new();
        s.put("auto", "k", Bytes::from("v")).await.map_err(|_| "put")?;
        assert!(s.exists("auto", "k").await.map_err(|_| "exists")?);
        Ok(())
    })
}

fn put_fails_when_auto_create_disabled() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let cfg = StorageConfig {
            auto_create_namespaces: false,
            ..Default::default()
        };
        let s = storage_with(cfg).await;
        let r = s.put("manual", "k", Bytes::from("v")).await;
        assert!(r.is_err());
        Ok(())
    })
}

fn get_fails_on_missing_namespace_without_auto_create() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let cfg = StorageConfig {
            auto_create_namespaces: false,
            ..Default::default()
        };
        let s = storage_with(cfg).await;
        let r = s.get("nowhere", "k").await;
        assert!(r.is_err());
        Ok(())
    })
}

fn namespace_prefix_isolates_storage() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let a = storage_with(StorageConfig::default().with_prefix("tenant_a")).await;
        let b = storage_with(StorageConfig::default().with_prefix("tenant_b")).await;

        a.put("ns", "k", Bytes::from("from_a")).await.map_err(|_| "put a")?;
        b.put("ns", "k", Bytes::from("from_b")).await.map_err(|_| "put b")?;

        assert_eq!(
            a.get("ns", "k").await.map_err(|_| "get a")?,
            Some(Bytes::from("from_a"))
        );
        assert_eq!(
            b.get("ns", "k").await.map_err(|_| "get b")?,
            Some(Bytes::from("from_b"))
        );
        Ok(())
    })
}

fn namespace_prefix_list_namespaces_is_stripped() -> Result<(), &'static str> {
    saikuro_tests::block_on(async {
        let s = storage_with(StorageConfig::default().with_prefix("app")).await;
        s.put("myns", "k", Bytes::from("v")).await.map_err(|_| "put")?;
        let nss = s.list_namespaces().await.map_err(|_| "list_namespaces")?;
        assert_eq!(nss, vec![saikuro_tests::String::from("myns")]);
        Ok(())
    })
}

fn supports_files_is_false() -> Result<(), &'static str> {
    assert!(!InMemoryStorage::new().supports_files());
    Ok(())
}

fn as_file_backend_is_none() -> Result<(), &'static str> {
    assert!(InMemoryStorage::new().as_file_backend().is_none());
    Ok(())
}