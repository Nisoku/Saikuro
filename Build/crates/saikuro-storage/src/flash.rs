//! Bounded key-value store over an async NOR-flash device.
//!
//! The store owns a contiguous region of [`NorFlash`] storage and lays it out
//! as a ring of `sector_count` erasable sectors. A sector starts with a u64
//! sequence number (all-`0xFF` = empty) followed by appended records. Every
//! record carries a CRC-32 so torn writes are detected and truncated on the
//! next [`open`](FlashKvStore::open).
//!
//! Writes are log-structured: `put` and `delete` append a record to the
//! active sector; deletion appends a tombstone. When the active sector fills,
//! the store advances to the next sector in the ring. If that sector still
//! holds data, the store compacts: the live set (materialized in RAM as the
//! running index) is erased and rewritten into a fresh generation, reserving
//! one sector as the spare that guarantees the next roll has room.
//!
//! # Limits
//!
//! The Tier 2 orchestrator bounds every persisted value through
//! [`FlashConfig`]: namespaces are at most 255 bytes, keys at most
//! `max_key_len` (default 64), values at most `max_value_len` (default
//! 4096), and a record must fit inside one sector. Usable capacity is
//! `(sector_count - 1) * (sector_size - sector_header)` bytes; exceeding it
//! returns [`StorageError::QuotaExceeded`]. The running index holds the whole
//! live set in RAM, so RAM usage tracks stored bytes.
//!
//! # Reliability
//!
//! Power loss during a normal append loses at most the torn tail record
//! (CRC-checked on open). Power loss during compaction loses the store: the
//! whole region is erased before the new generation is written. A
//! crash-consistent compaction journal is future work.
//!
//! # Concurrency
//!
//! The store is single-threaded and not reentrant, like the rest of the local
//! storage tier. Flash I/O borrows the internal `RefCell` across `await`, so
//! concurrent access from multiple tasks must be serialized by the
//! application (for example behind an embassy `Mutex`); a reentrant call
//! panics on the borrow instead of corrupting the log.

use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use bytes::Bytes;
use core::cell::RefCell;
use embedded_storage_async::nor_flash::NorFlash;

use super::{
    config::{limits, FlashConfig, StorageConfig},
    error::{Result, StorageError},
    traits::{LocalKeyValueBackend, LocalStorageBackend},
    util::{apply_prefix, strip_prefix},
};

const RECORD_HEADER_LEN: usize = 12;
const SEQ_LEN: usize = 8;
const EMPTY_SEQ: u64 = u64::MAX;
const TYPE_PUT: u8 = 0;
const TYPE_DELETE: u8 = 1;
const TYPE_CREATE_NAMESPACE: u8 = 2;
const TYPE_DELETE_NAMESPACE: u8 = 3;

fn align_up(n: usize, align: usize) -> usize {
    n.div_ceil(align) * align
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[derive(Default)]
struct Namespace {
    keys: BTreeMap<String, Vec<u8>>,
}

#[derive(Default)]
struct Index {
    namespaces: BTreeMap<String, Namespace>,
}

#[derive(Clone, Copy)]
struct Active {
    idx: usize,
    seq: u64,
    next_write: usize,
}

struct Record {
    typ: u8,
    ns: String,
    key: String,
    value: Vec<u8>,
}

struct FlashLog<F> {
    flash: F,
    flash_config: FlashConfig,
    index: Index,
    active: Active,
    opened: bool,
}

impl<F: NorFlash> FlashLog<F> {
    fn header_len(&self) -> usize {
        align_up(SEQ_LEN, F::WRITE_SIZE)
    }

    fn sector_start(&self, idx: usize) -> usize {
        self.flash_config.base_offset as usize + idx * self.flash_config.sector_size
    }

    fn sector_capacity(&self) -> usize {
        self.flash_config.sector_size - self.header_len()
    }

    fn encode_record(&self, typ: u8, ns: &str, key: &str, value: &[u8]) -> Result<Vec<u8>> {
        if ns.len() > limits::MAX_NAMESPACE_LEN {
            return Err(StorageError::internal(format!(
                "namespace exceeds {} bytes: {ns}",
                limits::MAX_NAMESPACE_LEN
            )));
        }
        if key.len() > self.flash_config.max_key_len {
            return Err(StorageError::internal(format!(
                "key exceeds {} bytes: {key}",
                self.flash_config.max_key_len
            )));
        }
        if value.len() > self.flash_config.max_value_len {
            return Err(StorageError::quota_exceeded(format!(
                "value exceeds {} bytes",
                self.flash_config.max_value_len
            )));
        }
        let mut header = Vec::with_capacity(RECORD_HEADER_LEN);
        header.push(typ);
        header.push(ns.len() as u8);
        header.extend_from_slice(&(key.len() as u16).to_le_bytes());
        header.extend_from_slice(&(value.len() as u32).to_le_bytes());

        let mut crc_buf = Vec::with_capacity(SEQ_LEN + ns.len() + key.len() + value.len());
        crc_buf.extend_from_slice(&header[..8]);
        crc_buf.extend_from_slice(ns.as_bytes());
        crc_buf.extend_from_slice(key.as_bytes());
        crc_buf.extend_from_slice(value);
        let crc = crc32(&crc_buf);

        let mut out = Vec::with_capacity(RECORD_HEADER_LEN + ns.len() + key.len() + value.len());
        out.extend_from_slice(&header[..]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(ns.as_bytes());
        out.extend_from_slice(key.as_bytes());
        out.extend_from_slice(value);
        Ok(out)
    }

    async fn read_exact(&mut self, offset: usize, buf: &mut [u8]) -> Result<()> {
        self.flash
            .read(offset as u32, buf)
            .await
            .map_err(|e| StorageError::internal(format!("flash read: {e:?}")))
    }

    async fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<()> {
        self.flash
            .write(offset as u32, bytes)
            .await
            .map_err(|e| StorageError::internal(format!("flash write: {e:?}")))
    }

    async fn erase_sector(&mut self, idx: usize) -> Result<()> {
        let base = self.sector_start(idx);
        self.flash
            .erase(base as u32, (base + self.flash_config.sector_size) as u32)
            .await
            .map_err(|e| StorageError::internal(format!("flash erase: {e:?}")))
    }

    async fn read_seq(&mut self, idx: usize) -> Result<u64> {
        let mut buf = [0xFFu8; SEQ_LEN];
        self.read_exact(self.sector_start(idx), &mut buf).await?;
        Ok(u64::from_le_bytes(buf))
    }

    async fn init_active(&mut self, idx: usize, seq: u64) -> Result<()> {
        let mut buf = vec![0xFFu8; self.header_len()];
        buf[..SEQ_LEN].copy_from_slice(&seq.to_le_bytes());
        self.write(self.sector_start(idx), &buf).await?;
        self.active = Active {
            idx,
            seq,
            next_write: self.header_len(),
        };
        Ok(())
    }

    async fn read_record(
        &mut self,
        sector: usize,
        offset: usize,
    ) -> Result<Option<(Record, usize)>> {
        let sector_size = self.flash_config.sector_size;
        if offset + RECORD_HEADER_LEN > sector_size {
            return Ok(None);
        }
        let mut hdr = [0u8; RECORD_HEADER_LEN];
        self.read_exact(self.sector_start(sector) + offset, &mut hdr)
            .await?;
        if hdr[0] == 0xFF {
            return Ok(None);
        }
        let ns_len = hdr[1] as usize;
        let key_len = u16::from_le_bytes([hdr[2], hdr[3]]) as usize;
        let value_len = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as usize;
        let stored_crc = u32::from_le_bytes([hdr[8], hdr[9], hdr[10], hdr[11]]);
        let payload_len = ns_len + key_len + value_len;

        if ns_len > limits::MAX_NAMESPACE_LEN
            || key_len > self.flash_config.max_key_len
            || value_len > self.flash_config.max_value_len
            || offset + RECORD_HEADER_LEN + payload_len > sector_size
        {
            return Ok(None);
        }

        let mut payload = vec![0u8; payload_len];
        self.read_exact(
            self.sector_start(sector) + offset + RECORD_HEADER_LEN,
            &mut payload,
        )
        .await?;
        let mut crc_buf = Vec::with_capacity(SEQ_LEN + payload_len);
        crc_buf.extend_from_slice(&hdr[..8]);
        crc_buf.extend_from_slice(&payload);
        if crc32(&crc_buf) != stored_crc {
            return Ok(None);
        }

        let rec = Record {
            typ: hdr[0],
            ns: String::from_utf8_lossy(&payload[..ns_len]).into_owned(),
            key: String::from_utf8_lossy(&payload[ns_len..ns_len + key_len]).into_owned(),
            value: payload[ns_len + key_len..].to_vec(),
        };
        let padded = align_up(RECORD_HEADER_LEN + payload_len, F::WRITE_SIZE);
        Ok(Some((rec, padded)))
    }

    fn apply_record(&mut self, rec: &Record, index: &mut Index) {
        match rec.typ {
            TYPE_PUT => {
                let ns = index.namespaces.entry(rec.ns.clone()).or_default();
                ns.keys.insert(rec.key.clone(), rec.value.clone());
            }
            TYPE_DELETE => {
                if let Some(ns) = index.namespaces.get_mut(&rec.ns) {
                    ns.keys.remove(&rec.key);
                }
            }
            TYPE_CREATE_NAMESPACE => {
                index.namespaces.entry(rec.ns.clone()).or_default();
            }
            TYPE_DELETE_NAMESPACE => {
                index.namespaces.remove(&rec.ns);
            }
            _ => {}
        }
    }

    async fn scan_sector(&mut self, sector: usize, index: &mut Index) -> Result<Option<Active>> {
        let seq = self.read_seq(sector).await?;
        if seq == EMPTY_SEQ {
            return Ok(None);
        }
        let mut offset = self.header_len();
        while let Some((rec, padded)) = self.read_record(sector, offset).await? {
            self.apply_record(&rec, index);
            offset += padded;
        }
        Ok(Some(Active {
            idx: sector,
            seq,
            next_write: offset,
        }))
    }

    async fn sector_occupied(&mut self, idx: usize) -> Result<bool> {
        Ok(self.read_seq(idx).await? != EMPTY_SEQ)
    }

    async fn write_record(&mut self, enc: &[u8]) -> Result<()> {
        let padded = align_up(enc.len(), F::WRITE_SIZE);
        let offset = self.sector_start(self.active.idx) + self.active.next_write;
        let mut buf = vec![0xFFu8; padded];
        buf[..enc.len()].copy_from_slice(enc);
        self.write(offset, &buf).await?;
        self.active.next_write += padded;
        Ok(())
    }

    /// Advance past the full active sector. Returns `true` when `pending` was
    /// already written by a compaction, `false` when the caller must append it
    /// into the freshly activated sector.
    async fn roll(&mut self, pending: &[u8]) -> Result<bool> {
        let next = (self.active.idx + 1) % self.flash_config.sector_count;
        if self.sector_occupied(next).await? {
            self.compact(pending).await?;
            Ok(true)
        } else {
            self.init_active(next, self.active.seq + 1).await?;
            Ok(false)
        }
    }

    async fn append_record(&mut self, enc: &[u8]) -> Result<()> {
        let padded = align_up(enc.len(), F::WRITE_SIZE);
        if padded > self.sector_capacity() {
            return Err(StorageError::quota_exceeded(
                "record does not fit in one sector",
            ));
        }
        if self.active.next_write + padded > self.flash_config.sector_size && self.roll(enc).await?
        {
            return Ok(());
        }
        self.write_record(enc).await
    }

    /// Erase the region and rewrite the live index as a fresh generation.
    ///
    /// `pending` is the record that could not be appended to the full active
    /// sector. A pending delete or namespace delete is folded into the
    /// compaction (the key/namespace is dropped from the output), so a
    /// shrinking operation never needs more space than the current live set.
    /// Growth operations (`put`, namespace markers) append `pending` after the
    /// live records and require the spare sector to survive, so they fail with
    /// `QuotaExceeded` instead of wedging the store.
    async fn compact(&mut self, pending: &[u8]) -> Result<()> {
        let pending_type = pending[0];
        let skip_key: Option<(String, String)> = if pending_type == TYPE_DELETE {
            let ns_len = pending[1] as usize;
            let key_len = u16::from_le_bytes([pending[2], pending[3]]) as usize;
            let ns = String::from_utf8_lossy(&pending[12..12 + ns_len]).into_owned();
            let key =
                String::from_utf8_lossy(&pending[12 + ns_len..12 + ns_len + key_len]).into_owned();
            Some((ns, key))
        } else {
            None
        };
        let skip_ns: Option<String> = if pending_type == TYPE_DELETE_NAMESPACE {
            let ns_len = pending[1] as usize;
            Some(String::from_utf8_lossy(&pending[12..12 + ns_len]).into_owned())
        } else {
            None
        };
        let shrinking = skip_key.is_some() || skip_ns.is_some();

        let mut records: Vec<Vec<u8>> = Vec::new();
        for (ns, namespace) in &self.index.namespaces {
            if let Some(ref skip) = skip_ns {
                if ns == skip {
                    continue;
                }
            }
            records.push(self.encode_record(TYPE_CREATE_NAMESPACE, ns, "", b"")?);
            for (key, value) in &namespace.keys {
                if let Some((ref skip_ns, ref skip_key)) = skip_key {
                    if ns == skip_ns && key == skip_key {
                        continue;
                    }
                }
                records.push(self.encode_record(TYPE_PUT, ns, key, value)?);
            }
        }
        if !shrinking {
            records.push(pending.to_vec());
        }

        let total: usize = records
            .iter()
            .map(|r| align_up(r.len(), F::WRITE_SIZE))
            .sum();
        let needed = total.div_ceil(self.sector_capacity());
        let spare = if shrinking { 0 } else { 1 };
        if needed + spare > self.flash_config.sector_count {
            return Err(StorageError::quota_exceeded("flash region full"));
        }

        for idx in 0..self.flash_config.sector_count {
            self.erase_sector(idx).await?;
        }

        let base_seq = self.active.seq + 1;
        let mut idx = 0usize;
        let mut seq = base_seq;
        self.init_active(idx, seq).await?;
        for record in &records {
            let padded = align_up(record.len(), F::WRITE_SIZE);
            if self.active.next_write + padded > self.flash_config.sector_size {
                idx = (idx + 1) % self.flash_config.sector_count;
                seq += 1;
                self.init_active(idx, seq).await?;
            }
            self.write_record(record).await?;
        }
        Ok(())
    }

    fn ensure_opened(&self) -> Result<()> {
        if self.opened {
            Ok(())
        } else {
            Err(StorageError::internal(
                "flash store is not opened; call open() first",
            ))
        }
    }

    async fn open(&mut self) -> Result<()> {
        let mut index = Index::default();
        let mut best: Option<Active> = None;
        for idx in 0..self.flash_config.sector_count {
            let scanned = self.scan_sector(idx, &mut index).await?;
            if let Some(active) = scanned {
                match best {
                    None => best = Some(active),
                    Some(b) if active.seq > b.seq => best = Some(active),
                    Some(_) => {}
                }
            }
        }
        self.index = index;
        self.active = match best {
            Some(active) => active,
            None => {
                // Fresh region: write sector 0's sequence header so a later
                // open recognizes the active sector.
                self.init_active(0, 0).await?;
                self.active
            }
        };
        self.opened = true;
        Ok(())
    }

    async fn ensure_namespace(&mut self, stored_ns: &str) -> Result<()> {
        let enc = self.encode_record(TYPE_CREATE_NAMESPACE, stored_ns, "", b"")?;
        self.append_record(&enc).await?;
        self.index
            .namespaces
            .entry(stored_ns.to_owned())
            .or_default();
        Ok(())
    }
}

/// A bounded, durable key-value store over an async NOR-flash device.
///
/// See the [module documentation](self) for the on-flash layout, size limits,
/// and reliability guarantees. The store owns the device and a RAM index of
/// the live set; the region is erased and rewritten by compaction, so this
/// backend never allocates beyond `max_value_len` per record plus the live
/// index.
pub struct FlashKvStore<F> {
    config: StorageConfig,
    flash_config: FlashConfig,
    inner: RefCell<FlashLog<F>>,
}

impl<F: NorFlash> FlashKvStore<F> {
    /// Validate the store geometry against the device and construct the
    /// store. Call [`open`](Self::open) before using it.
    pub fn new(flash: F, config: StorageConfig, flash_config: FlashConfig) -> Result<Self> {
        let geometry = FlashConfig::new(
            flash_config.base_offset,
            flash_config.sector_size,
            flash_config.sector_count,
            flash_config.max_key_len,
            flash_config.max_value_len,
            F::ERASE_SIZE,
        )
        .map_err(|msg| StorageError::internal(format!("invalid flash config: {msg}")))?;

        let header_len = align_up(SEQ_LEN, F::WRITE_SIZE);
        let record_max = align_up(
            RECORD_HEADER_LEN
                + limits::MAX_NAMESPACE_LEN
                + geometry.max_key_len
                + geometry.max_value_len,
            F::WRITE_SIZE,
        );
        if !(geometry.base_offset as usize).is_multiple_of(F::ERASE_SIZE) {
            return Err(StorageError::internal(
                "flash base offset must be aligned to the erase size",
            ));
        }
        if !geometry.sector_size.is_multiple_of(F::WRITE_SIZE) {
            return Err(StorageError::internal(
                "flash sector size must be a multiple of the write size",
            ));
        }
        if geometry.sector_size < header_len + RECORD_HEADER_LEN {
            return Err(StorageError::internal(
                "flash sector size too small for the record header",
            ));
        }
        if geometry.base_offset as usize + geometry.region_size() > flash.capacity() {
            return Err(StorageError::internal(
                "flash region exceeds the device capacity",
            ));
        }
        if record_max > geometry.sector_size - header_len {
            return Err(StorageError::internal(
                "a maximum-size record does not fit one sector",
            ));
        }

        Ok(Self {
            config,
            flash_config: geometry,
            inner: RefCell::new(FlashLog {
                flash,
                flash_config: geometry,
                index: Index::default(),
                active: Active {
                    idx: 0,
                    seq: 0,
                    next_write: header_len,
                },
                opened: false,
            }),
        })
    }

    /// Scan the region and rebuild the index. Safe to call again to recover
    /// from a torn tail left by a power loss during a normal append.
    pub async fn open(&mut self) -> Result<()> {
        self.inner.get_mut().open().await
    }

    /// The generic storage configuration.
    pub fn config(&self) -> &StorageConfig {
        &self.config
    }

    /// The flash geometry and size limits.
    pub fn flash_config(&self) -> FlashConfig {
        self.flash_config
    }

    /// Usable storage capacity in bytes: `(sector_count - 1)` sectors, the
    /// last one reserved as the compaction spare.
    pub fn capacity(&self) -> usize {
        let header_len = align_up(SEQ_LEN, F::WRITE_SIZE);
        (self.flash_config.sector_count - 1) * (self.flash_config.sector_size - header_len)
    }
}

// The borrow is held across await on purpose: the store is single-threaded
// and non-reentrant (see the module docs), and a reentrant call panics on the
// borrow instead of interleaving log writes.
#[allow(clippy::await_holding_refcell_ref)]
impl<F: NorFlash + 'static> LocalKeyValueBackend for FlashKvStore<F> {
    fn config(&self) -> &StorageConfig {
        &self.config
    }

    async fn exists(&self, namespace: &str, key: &str) -> Result<bool> {
        let inner = self.inner.borrow_mut();
        inner.ensure_opened()?;
        let stored_ns = apply_prefix(&self.config, namespace);
        match inner.index.namespaces.get(&stored_ns) {
            Some(ns) => Ok(ns.keys.contains_key(key)),
            None if self.config.auto_create_namespaces => Ok(false),
            None => Err(StorageError::namespace_not_found(namespace)),
        }
    }

    async fn get(&self, namespace: &str, key: &str) -> Result<Option<Bytes>> {
        let inner = self.inner.borrow_mut();
        inner.ensure_opened()?;
        let stored_ns = apply_prefix(&self.config, namespace);
        match inner.index.namespaces.get(&stored_ns) {
            Some(ns) => Ok(ns.keys.get(key).map(|v| Bytes::from(v.clone()))),
            None if self.config.auto_create_namespaces => Ok(None),
            None => Err(StorageError::namespace_not_found(namespace)),
        }
    }

    async fn put(&self, namespace: &str, key: &str, value: Bytes) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner.ensure_opened()?;
        let stored_ns = apply_prefix(&self.config, namespace);
        if !inner.index.namespaces.contains_key(&stored_ns) {
            if !self.config.auto_create_namespaces {
                return Err(StorageError::namespace_not_found(namespace));
            }
            inner.ensure_namespace(&stored_ns).await?;
        }
        let enc = inner.encode_record(TYPE_PUT, &stored_ns, key, &value)?;
        inner.append_record(&enc).await?;
        let ns = inner
            .index
            .namespaces
            .get_mut(&stored_ns)
            .expect("namespace ensured above");
        ns.keys.insert(key.to_owned(), value.to_vec());
        Ok(())
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner.ensure_opened()?;
        let stored_ns = apply_prefix(&self.config, namespace);
        let present = match inner.index.namespaces.get(&stored_ns) {
            None => return Ok(()),
            Some(ns) => ns.keys.contains_key(key),
        };
        if !present {
            return Ok(());
        }
        let enc = inner.encode_record(TYPE_DELETE, &stored_ns, key, b"")?;
        inner.append_record(&enc).await?;
        let ns = inner
            .index
            .namespaces
            .get_mut(&stored_ns)
            .expect("namespace present above");
        ns.keys.remove(key);
        Ok(())
    }

    async fn list_keys(&self, namespace: &str) -> Result<Vec<String>> {
        let inner = self.inner.borrow_mut();
        inner.ensure_opened()?;
        let stored_ns = apply_prefix(&self.config, namespace);
        match inner.index.namespaces.get(&stored_ns) {
            Some(ns) => Ok(ns.keys.keys().cloned().collect()),
            None if self.config.auto_create_namespaces => Ok(Vec::new()),
            None => Err(StorageError::namespace_not_found(namespace)),
        }
    }

    async fn list_namespaces(&self) -> Result<Vec<String>> {
        let inner = self.inner.borrow_mut();
        inner.ensure_opened()?;
        Ok(inner
            .index
            .namespaces
            .keys()
            .map(|ns| strip_prefix(&self.config, ns))
            .collect())
    }

    async fn create_namespace(&self, namespace: &str) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner.ensure_opened()?;
        let stored_ns = apply_prefix(&self.config, namespace);
        if inner.index.namespaces.contains_key(&stored_ns) {
            return Err(StorageError::namespace_already_exists(namespace));
        }
        inner.ensure_namespace(&stored_ns).await?;
        Ok(())
    }

    async fn delete_namespace(&self, namespace: &str) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner.ensure_opened()?;
        let stored_ns = apply_prefix(&self.config, namespace);
        if !inner.index.namespaces.contains_key(&stored_ns) {
            return Ok(());
        }
        let enc = inner.encode_record(TYPE_DELETE_NAMESPACE, &stored_ns, "", b"")?;
        inner.append_record(&enc).await?;
        inner.index.namespaces.remove(&stored_ns);
        Ok(())
    }

    async fn clear_namespace(&self, namespace: &str) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner.ensure_opened()?;
        let stored_ns = apply_prefix(&self.config, namespace);
        let keys: Vec<String> = match inner.index.namespaces.get(&stored_ns) {
            Some(ns) => ns.keys.keys().cloned().collect(),
            None => return Ok(()),
        };
        for key in keys {
            let enc = inner.encode_record(TYPE_DELETE, &stored_ns, &key, b"")?;
            inner.append_record(&enc).await?;
            let ns = inner
                .index
                .namespaces
                .get_mut(&stored_ns)
                .expect("namespace present above");
            ns.keys.remove(&key);
        }
        Ok(())
    }
}

impl<F: NorFlash + 'static> LocalStorageBackend for FlashKvStore<F> {
    fn supports_files(&self) -> bool {
        false
    }
}
