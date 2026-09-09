use crate::shared_test;
use crate::TestSuite;
use saikuro_storage::KeyValueBackend;
use saikuro_storage::SqliteStorage;

pub fn register(suite: &mut TestSuite) {
    shared_test!(suite, "sqlite::temporary", temporary);
    shared_test!(suite, "sqlite::put_get", put_get);
    shared_test!(suite, "sqlite::delete", delete);
    shared_test!(suite, "sqlite::list_keys", list_keys);
    shared_test!(suite, "sqlite::namespaces", namespaces);
    shared_test!(suite, "sqlite::exists", exists);
    shared_test!(suite, "sqlite::overwrite", overwrite);
}

fn temporary() -> Result<(), &'static str> {
    let _db = SqliteStorage::temporary().map_err(|_| "temp sqlite")?;
    Ok(())
}

fn put_get() -> Result<(), &'static str> {
    let db = SqliteStorage::temporary().map_err(|_| "temp sqlite")?;
    crate::block_on(async move {
        db.put("ns", "key1", bytes::Bytes::from_static(b"hello"))
            .await
            .map_err(|_| "put")?;
        let val = db.get("ns", "key1").await.map_err(|_| "get")?;
        assert_eq!(val.as_deref(), Some(b"hello".as_slice()));
        Ok(())
    })
}

fn delete() -> Result<(), &'static str> {
    let db = SqliteStorage::temporary().map_err(|_| "temp sqlite")?;
    crate::block_on(async move {
        db.put("ns", "k", bytes::Bytes::from_static(b"data"))
            .await
            .map_err(|_| "put")?;
        db.delete("ns", "k").await.map_err(|_| "delete")?;
        let val = db.get("ns", "k").await.map_err(|_| "get")?;
        assert!(val.is_none());
        Ok(())
    })
}

fn list_keys() -> Result<(), &'static str> {
    let db = SqliteStorage::temporary().map_err(|_| "temp sqlite")?;
    crate::block_on(async move {
        db.put("ns", "a", bytes::Bytes::from_static(b"1"))
            .await
            .map_err(|_| "put")?;
        db.put("ns", "b", bytes::Bytes::from_static(b"2"))
            .await
            .map_err(|_| "put")?;
        db.put("ns", "c", bytes::Bytes::from_static(b"3"))
            .await
            .map_err(|_| "put")?;
        let mut keys = db.list_keys("ns").await.map_err(|_| "list_keys")?;
        keys.sort();
        assert_eq!(keys, vec!["a", "b", "c"]);
        Ok(())
    })
}

fn namespaces() -> Result<(), &'static str> {
    let db = SqliteStorage::temporary().map_err(|_| "temp sqlite")?;
    crate::block_on(async move {
        db.put("alpha", "k", bytes::Bytes::from_static(b"1"))
            .await
            .map_err(|_| "put")?;
        db.put("beta", "k", bytes::Bytes::from_static(b"2"))
            .await
            .map_err(|_| "put")?;
        let mut ns = db.list_namespaces().await.map_err(|_| "namespaces")?;
        ns.sort();
        assert_eq!(ns, vec!["alpha", "beta"]);
        Ok(())
    })
}

fn exists() -> Result<(), &'static str> {
    let db = SqliteStorage::temporary().map_err(|_| "temp sqlite")?;
    crate::block_on(async move {
        assert!(!db.exists("ns", "k").await.map_err(|_| "exists")?);
        db.put("ns", "k", bytes::Bytes::from_static(b"v"))
            .await
            .map_err(|_| "put")?;
        assert!(db.exists("ns", "k").await.map_err(|_| "exists")?);
        Ok(())
    })
}

fn overwrite() -> Result<(), &'static str> {
    let db = SqliteStorage::temporary().map_err(|_| "temp sqlite")?;
    crate::block_on(async move {
        db.put("ns", "k", bytes::Bytes::from_static(b"v1"))
            .await
            .map_err(|_| "put")?;
        db.put("ns", "k", bytes::Bytes::from_static(b"v2"))
            .await
            .map_err(|_| "put")?;
        let val = db.get("ns", "k").await.map_err(|_| "get")?;
        assert_eq!(val.as_deref(), Some(b"v2".as_slice()));
        Ok(())
    })
}
