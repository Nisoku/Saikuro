use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use bytes::Bytes;
use embedded_storage_async::nor_flash::NorFlash;
use sequential_storage::cache::Cache;
use sequential_storage::map::{MapConfig, MapStorage};
use sequential_storage::Error as SsError;

use crate::config::limits::MAX_NAMESPACE_LEN;
use crate::config::{FlashConfig, StorageConfig};
use crate::traits::{KeyValueBackend, StorageBackend};
use crate::util::{apply_prefix, strip_prefix};
use crate::{Result, SaikuroError};

/// Tag byte for a namespace-existence marker key.
const TAG_NAMESPACE: u8 = 0x00;
/// Tag byte for a data record key.
const TAG_DATA: u8 = 0x01;

/// Upper bound on a single `sequential_storage` item (key + value lengths are
/// encoded as `u16`), so an item can never exceed 64 KiB.
const MAX_ITEM_LEN: usize = 0xFFFF;

/// Flash-backed key-value store.
pub struct FlashKvStore<F: NorFlash> {
    config: StorageConfig,
    flash_config: FlashConfig,
    inner: core::cell::RefCell<MapStorage<Vec<u8>, F, Cache<Vec<u8>>>>,
}

impl<F: NorFlash> FlashKvStore<F> {
    /// Construct a store over `flash` with the given geometry and size limits.
    ///
    /// Performs device-specific validation that `sequential_storage` cannot do
    /// for itself: region alignment, at least two erase pages, region within
    /// device capacity, and per-item sizes inside the 64 KiB cap.
    pub fn new(flash: F, config: StorageConfig, flash_config: FlashConfig) -> Result<Self> {
        let erase = F::ERASE_SIZE;
        let region = flash_config.region_size();
        let capacity = flash.capacity();

        if flash_config.base_offset as usize % erase != 0 {
            return Err(SaikuroError::internal(format!(
                "base_offset {} is not aligned to erase size {erase}",
                flash_config.base_offset
            )));
        }
        if flash_config.sector_size == 0 || flash_config.sector_size % erase != 0 {
            return Err(SaikuroError::internal(format!(
                "sector_size {} is not a positive multiple of erase size {erase}",
                flash_config.sector_size
            )));
        }
        if flash_config.sector_count < 2 {
            return Err(SaikuroError::internal(
                "flash region needs at least two sectors (one spare for compaction)",
            ));
        }
        if region > capacity {
            return Err(SaikuroError::internal(format!(
                "flash region size {region} exceeds device capacity {capacity}"
            )));
        }
        if flash_config.max_key_len == 0 || flash_config.max_key_len > MAX_ITEM_LEN {
            return Err(SaikuroError::internal(format!(
                "max_key_len {} out of range (1..={MAX_ITEM_LEN})",
                flash_config.max_key_len
            )));
        }
        if flash_config.max_value_len == 0 || flash_config.max_value_len > MAX_ITEM_LEN {
            return Err(SaikuroError::internal(format!(
                "max_value_len {} out of range (1..={MAX_ITEM_LEN})",
                flash_config.max_value_len
            )));
        }

        let start = flash_config.base_offset;
        let end = start + region as u32;
        let map_config = MapConfig::<F>::try_new(start..end).map_err(|_| {
            SaikuroError::internal("invalid sequential-storage region (alignment or size)")
        })?;

        let inner = MapStorage::<Vec<u8>, F, Cache<Vec<u8>>>::new(
            flash,
            map_config,
            Cache::new_uncached(),
        );

        Ok(Self {
            config,
            flash_config,
            inner: core::cell::RefCell::new(inner),
        })
    }

    /// A scratch buffer large enough for the largest permitted item, aligned to
    /// the device write word.
    fn scratch_buf(&self) -> Vec<u8> {
        let len = (1
            + 4
            + MAX_NAMESPACE_LEN
            + self.flash_config.max_key_len
            + 1
            + 4
            + self.flash_config.max_value_len
            + 32)
            .next_multiple_of(F::WRITE_SIZE);
        alloc::vec![0u8; len]
    }

    /// Returns `true` if `stored_ns` has a live namespace marker.
    #[allow(clippy::await_holding_refcell_ref)]
    async fn namespace_exists(&self, stored_ns: &str) -> Result<bool> {
        let mut inner = self.inner.borrow_mut();
        let mut buf = self.scratch_buf();
        let marker = make_marker_bytes(stored_ns);
        let existing = inner
            .fetch_item::<Option<Vec<u8>>>(&mut buf, &marker)
            .await
            .map_err(map_err)?;
        Ok(existing.is_some())
    }
}

impl<F: NorFlash> KeyValueBackend for FlashKvStore<F>
where
    F: 'static,
{
    fn config(&self) -> &StorageConfig {
        &self.config
    }

    #[allow(clippy::await_holding_refcell_ref)]
    async fn exists(&self, namespace: &str, key: &str) -> Result<bool> {
        Ok(self.get(namespace, key).await?.is_some())
    }

    #[allow(clippy::await_holding_refcell_ref)]
    async fn get(&self, namespace: &str, key: &str) -> Result<Option<Bytes>> {
        let stored_ns = apply_prefix(&self.config, namespace);
        if !self.namespace_exists(&stored_ns).await? {
            if self.config.auto_create_namespaces {
                return Ok(None);
            }
            return Err(SaikuroError::namespace_not_found(namespace.to_string()));
        }

        let mut inner = self.inner.borrow_mut();
        let mut buf = self.scratch_buf();
        let k = make_key_bytes(&stored_ns, key);
        let value = inner
            .fetch_item::<Option<Vec<u8>>>(&mut buf, &k)
            .await
            .map_err(map_err)?;
        Ok(value.flatten().map(Bytes::from))
    }

    #[allow(clippy::await_holding_refcell_ref)]
    async fn put(&self, namespace: &str, key: &str, value: Bytes) -> Result<()> {
        let stored_ns = apply_prefix(&self.config, namespace);
        if stored_ns.len() > MAX_NAMESPACE_LEN {
            return Err(SaikuroError::internal(format!(
                "namespace too long: {} bytes (max {MAX_NAMESPACE_LEN})",
                stored_ns.len()
            )));
        }
        if key.len() > self.flash_config.max_key_len {
            return Err(SaikuroError::internal(format!(
                "key too long: {} bytes (max {})",
                key.len(),
                self.flash_config.max_key_len
            )));
        }
        if value.len() > self.flash_config.max_value_len {
            return Err(SaikuroError::quota_exceeded(format!(
                "value too large: {} bytes (max {})",
                value.len(),
                self.flash_config.max_value_len
            )));
        }

        let mut inner = self.inner.borrow_mut();
        let mut buf = self.scratch_buf();
        let marker = make_marker_bytes(&stored_ns);
        let existing = inner
            .fetch_item::<Option<Vec<u8>>>(&mut buf, &marker)
            .await
            .map_err(map_err)?;
        if existing.is_none() {
            if !self.config.auto_create_namespaces {
                return Err(SaikuroError::namespace_not_found(namespace.to_string()));
            }
            inner
                .store_item(&mut buf, &marker, &Some(Vec::new()))
                .await
                .map_err(map_err)?;
        }

        let k = make_key_bytes(&stored_ns, key);
        inner
            .store_item(&mut buf, &k, &Some(value.to_vec()))
            .await
            .map_err(map_err)?;
        Ok(())
    }

    #[allow(clippy::await_holding_refcell_ref)]
    async fn delete(&self, namespace: &str, key: &str) -> Result<()> {
        let stored_ns = apply_prefix(&self.config, namespace);
        if !self.namespace_exists(&stored_ns).await? {
            if self.config.auto_create_namespaces {
                return Ok(());
            }
            return Err(SaikuroError::namespace_not_found(namespace.to_string()));
        }

        let mut inner = self.inner.borrow_mut();
        let mut buf = self.scratch_buf();
        let k = make_key_bytes(&stored_ns, key);
        let value = inner
            .fetch_item::<Option<Vec<u8>>>(&mut buf, &k)
            .await
            .map_err(map_err)?;
        if value.flatten().is_some() {
            inner
                .store_item(&mut buf, &k, &None::<Vec<u8>>)
                .await
                .map_err(map_err)?;
        }
        Ok(())
    }

    #[allow(clippy::await_holding_refcell_ref)]
    async fn list_keys(&self, namespace: &str) -> Result<Vec<String>> {
        let stored_ns = apply_prefix(&self.config, namespace);
        if !self.namespace_exists(&stored_ns).await? {
            return Err(SaikuroError::namespace_not_found(namespace.to_string()));
        }

        let mut inner = self.inner.borrow_mut();
        let mut buf = self.scratch_buf();
        let mut iter = inner
            .fetch_all_items(&mut buf)
            .await
            .map_err(map_err)?;

        let mut out: Vec<String> = Vec::new();
        while let Some((k, v)) = iter
            .next::<Option<Vec<u8>>>(&mut buf)
            .await
            .map_err(map_err)?
        {
            if let Some((TAG_DATA, ns, key)) = parse_key(&k) {
                if ns == stored_ns && v.is_some() {
                    out.push(key.to_string());
                }
            }
        }
        Ok(out)
    }

    #[allow(clippy::await_holding_refcell_ref)]
    async fn list_namespaces(&self) -> Result<Vec<String>> {
        let mut inner = self.inner.borrow_mut();
        let mut buf = self.scratch_buf();
        let mut iter = inner
            .fetch_all_items(&mut buf)
            .await
            .map_err(map_err)?;

        let mut live: BTreeSet<String> = BTreeSet::new();
        while let Some((k, v)) = iter
            .next::<Option<Vec<u8>>>(&mut buf)
            .await
            .map_err(map_err)?
        {
            if v.is_none() {
                continue;
            }
            if let Some((_, ns, _)) = parse_key(&k) {
                live.insert(ns.to_string());
            }
        }

        Ok(live
            .into_iter()
            .map(|ns| strip_prefix(&self.config, &ns))
            .collect())
    }

    #[allow(clippy::await_holding_refcell_ref)]
    async fn create_namespace(&self, namespace: &str) -> Result<()> {
        let stored_ns = apply_prefix(&self.config, namespace);
        if stored_ns.len() > MAX_NAMESPACE_LEN {
            return Err(SaikuroError::internal(format!(
                "namespace too long: {} bytes (max {MAX_NAMESPACE_LEN})",
                stored_ns.len()
            )));
        }

        let mut inner = self.inner.borrow_mut();
        let mut buf = self.scratch_buf();
        let marker = make_marker_bytes(&stored_ns);
        let existing = inner
            .fetch_item::<Option<Vec<u8>>>(&mut buf, &marker)
            .await
            .map_err(map_err)?;
        if existing.is_some() {
            return Err(SaikuroError::namespace_already_exists(namespace.to_string()));
        }
        inner
            .store_item(&mut buf, &marker, &Some(Vec::new()))
            .await
            .map_err(map_err)?;
        Ok(())
    }

    #[allow(clippy::await_holding_refcell_ref)]
    async fn delete_namespace(&self, namespace: &str) -> Result<()> {
        let stored_ns = apply_prefix(&self.config, namespace);

        let mut inner = self.inner.borrow_mut();
        let mut buf = self.scratch_buf();

        let mut to_tombstone: Vec<Vec<u8>> = Vec::new();
        {
            let mut iter = inner
                .fetch_all_items(&mut buf)
                .await
                .map_err(map_err)?;
            while let Some((k, v)) = iter
                .next::<Option<Vec<u8>>>(&mut buf)
                .await
                .map_err(map_err)?
            {
                if let Some((TAG_DATA, ns, _)) = parse_key(&k) {
                    if ns == stored_ns && v.is_some() {
                        to_tombstone.push(k);
                    }
                }
            }
        }
        // The iterator borrows `inner`; drop it before we start writing.
        // Also drop the namespace marker so the namespace no longer appears in
        // `list_namespaces`. Tombstoning a missing marker is harmless.
        to_tombstone.push(make_marker_bytes(&stored_ns));

        for key in to_tombstone {
            inner
                .store_item(&mut buf, &key, &None::<Vec<u8>>)
                .await
                .map_err(map_err)?;
        }
        Ok(())
    }

    #[allow(clippy::await_holding_refcell_ref)]
    async fn clear_namespace(&self, namespace: &str) -> Result<()> {
        let stored_ns = apply_prefix(&self.config, namespace);
        if !self.namespace_exists(&stored_ns).await? {
            return Ok(());
        }

        let mut inner = self.inner.borrow_mut();
        let mut buf = self.scratch_buf();

        let mut to_tombstone: Vec<Vec<u8>> = Vec::new();
        {
            let mut iter = inner
                .fetch_all_items(&mut buf)
                .await
                .map_err(map_err)?;
            while let Some((k, v)) = iter
                .next::<Option<Vec<u8>>>(&mut buf)
                .await
                .map_err(map_err)?
            {
                if let Some((TAG_DATA, ns, _)) = parse_key(&k) {
                    if ns == stored_ns && v.is_some() {
                        to_tombstone.push(k);
                    }
                }
            }
        }
        // The iterator borrows `inner`; drop it before we start writing.
        for key in to_tombstone {
            inner
                .store_item(&mut buf, &key, &None::<Vec<u8>>)
                .await
                .map_err(map_err)?;
        }
        Ok(())
    }
}

impl<F: NorFlash> StorageBackend for FlashKvStore<F>
where
    F: 'static,
{
    fn supports_files(&self) -> bool {
        false
    }
}

/// Build a namespace marker key: `[0x00] || ns_len:u32 (LE) || ns`.
fn make_marker_bytes(stored_ns: &str) -> Vec<u8> {
    let nb = stored_ns.as_bytes();
    let mut v = Vec::with_capacity(1 + 4 + nb.len());
    v.push(TAG_NAMESPACE);
    v.extend_from_slice(&(nb.len() as u32).to_le_bytes());
    v.extend_from_slice(nb);
    v
}

/// Build a data record key: `[0x01] || ns_len:u32 (LE) || ns || key`.
fn make_key_bytes(stored_ns: &str, key: &str) -> Vec<u8> {
    let nb = stored_ns.as_bytes();
    let kb = key.as_bytes();
    let mut v = Vec::with_capacity(1 + 4 + nb.len() + kb.len());
    v.push(TAG_DATA);
    v.extend_from_slice(&(nb.len() as u32).to_le_bytes());
    v.extend_from_slice(nb);
    v.extend_from_slice(kb);
    v
}

/// Parse a stored key into `(tag, namespace, key)` where `key` is empty for a
/// namespace marker. Returns `None` for malformed keys.
fn parse_key(k: &[u8]) -> Option<(u8, &str, &str)> {
    if k.len() < 5 {
        return None;
    }
    let tag = k[0];
    let ns_len = u32::from_le_bytes([k[1], k[2], k[3], k[4]]) as usize;
    let rest = &k[5..];
    if rest.len() < ns_len {
        return None;
    }
    let ns = core::str::from_utf8(&rest[..ns_len]).ok()?;
    let key = core::str::from_utf8(&rest[ns_len..]).ok()?;
    Some((tag, ns, key))
}

/// Map a `sequential_storage` error into the crate error type.
fn map_err<E: core::fmt::Debug>(e: SsError<E>) -> SaikuroError {
    use SsError::*;
    match e {
        Storage { value } => {
            SaikuroError::internal(format!("flash I/O error: {value:?}"))
        }
        FullStorage => SaikuroError::quota_exceeded("flash region is full"),
        Corrupted { .. } => SaikuroError::internal("flash region is corrupted"),
        LogicBug { .. } => SaikuroError::internal("flash storage logic bug"),
        BufferTooBig => SaikuroError::internal("scratch buffer too large"),
        BufferTooSmall(n) => SaikuroError::internal(format!(
            "scratch buffer too small (need {n} bytes)"
        )),
        SerializationError(_) => SaikuroError::internal("serialization error"),
        ItemTooBig => SaikuroError::internal("item exceeds the 64 KiB flash limit"),
        _ => SaikuroError::internal("unknown flash storage error"),
    }
}
