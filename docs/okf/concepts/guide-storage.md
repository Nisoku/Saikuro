---
type: concept
title: Storage
description: "Platform-agnostic storage backends for key-value and file operations"
source: "https://nisoku.org/Saikuro/guide/storage/"
path: /guide/storage/
updated: 2026-07-15
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-07-15T17:26:24.478Z"
---
---
title: "Storage"
description: "Platform-agnostic storage backends for key-value and file operations"
---

The `saikuro-storage` crate provides a unified storage abstraction that works across native and WASM environments. All backends share the same traits, so switching between them is a configuration change.

## Storage Traits

### KeyValueBackend

The core key-value interface with namespace support:

```rust
#[async_trait]
pub trait KeyValueBackend {
    async fn exists(&self, namespace: &str, key: &str) -> Result<bool>;
    async fn get(&self, namespace: &str, key: &str) -> Result<Option<Bytes>>;
    async fn put(&self, namespace: &str, key: &str, value: Bytes) -> Result<()>;
    async fn delete(&self, namespace: &str, key: &str) -> Result<()>;
    async fn list_keys(&self, namespace: &str) -> Result<Vec<String>>;
    async fn list_namespaces(&self) -> Result<Vec<String>>;
    async fn create_namespace(&self, namespace: &str) -> Result<()>;
    async fn delete_namespace(&self, namespace: &str) -> Result<()>;
    async fn clear_namespace(&self, namespace: &str) -> Result<()>;
}
```

### FileBackend

Hierarchical file-like operations:

```rust
#[async_trait]
pub trait FileBackend {
    async fn read_file(&self, path: &str) -> Result<Bytes>;
    async fn write_file(&self, path: &str, content: Bytes) -> Result<()>;
    async fn append_file(&self, path: &str, content: Bytes) -> Result<()>;
    async fn delete_file(&self, path: &str) -> Result<()>;
    async fn file_exists(&self, path: &str) -> Result<bool>;
    async fn list_dir(&self, path: &str) -> Result<Vec<String>>;
    async fn create_dir(&self, path: &str) -> Result<()>;
    async fn delete_dir(&self, path: &str) -> Result<()>;
}
```

### KeyValueBackendExt

Extension methods for JSON and MessagePack serialization:

```rust
use saikuro_storage::KeyValueBackendExt;

// Automatic JSON round-trip
let user: Option<User> = storage.get_json("users", "alice").await?;
storage.put_json("users", "alice", &user).await?;

// Automatic MessagePack round-trip
let user: Option<User> = storage.get_msgpack("users", "alice").await?;
storage.put_msgpack("users", "alice", &user).await?;
```

## Available Backends

| Backend        | Feature Flag      | Class               | Platform       |
|----------------|-------------------|---------------------|----------------|
| InMemory       | `inmemory`        | `InMemoryStorage`   | All            |
| Filesystem     | `fs-storage`      | `FilesystemStorage` | Native         |
| SQLite         | `sqlite-storage`  | `SqliteStorage`     | Native         |
| Sled           | `sled-storage`    | `SledStorage`       | Native         |
| IndexedDB      | `wasm-storage`    | `IndexedDbStorage`  | WASM (browser) |
| OPFS           | `wasm-storage`    | `OpfsStorage`       | WASM (browser) |
| LocalStorage   | `local-storage`   | `LocalStorage`      | WASM (browser) |
| SessionStorage | `session-storage` | `SessionStorage`    | WASM (browser) |
| FsAccess       | `wasm-storage`    | `FsAccessStorage`   | WASM (browser) |

## Usage

```rust
use saikuro_storage::{InMemoryStorage, KeyValueBackend, KeyValueBackendExt};
use saikuro_storage::config::StorageConfig;

let storage = InMemoryStorage::with_config(StorageConfig::default());

// Write
storage.put("myapp", "config", b"value".into()).await?;

// Read
let val = storage.get("myapp", "config").await?;

// JSON
storage.put_json("myapp", "settings", &Settings { theme: "dark" }).await?;
let settings: Option<Settings> = storage.get_json("myapp", "settings").await?;
```

## Configuration

```rust
use saikuro_storage::config::{StorageConfig, BackendKind, PersistenceMode, CleanupPolicy};

let config = StorageConfig::default()
    .with_namespace_prefix("myapp")
    .with_persistence(PersistenceMode::Persistent)
    .with_cleanup(CleanupPolicy::Ttl(std::time::Duration::from_secs(3600)));
```

## Next Steps

::: grids
::: grid
::: button "WASM Guide" ./wasm.md icon:globe
:::
::: grid
::: button "Transports" ./transports.md icon:radio
:::
::: grid
::: button "Rust Adapter" ../adapters/rust/ icon:box
:::
:::
