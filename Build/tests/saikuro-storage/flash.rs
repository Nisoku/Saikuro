//! Integration tests for the flash-backed bounded key-value store.
//!
//! The fake NOR-flash device enforces real NOR semantics: aligned reads and
//! writes, per-word write-once, erase-to-`0xFF`, and `1`-only-to-`0` bit
//! transitions. Tests share the fake behind `Rc<RefCell<...>>` so a "reboot"
//! is a fresh store opened over the same device contents.

use std::cell::RefCell;
use std::rc::Rc;

use bytes::Bytes;
use embedded_storage_async::nor_flash::{
    ErrorType, NorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash,
};
use futures_executor::block_on;
use saikuro_storage::{
    FlashConfig, FlashKvStore, LocalKeyValueBackend, StorageConfig, StorageError,
};

const WRITE_SIZE: usize = 4;
const ERASE_SIZE: usize = 256;
const REGION_SIZE: usize = 512 * 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FlashTestError {
    kind: NorFlashErrorKind,
}

impl ErrorType for FakeFlash {
    type Error = FlashTestError;
}

impl NorFlashError for FlashTestError {
    fn kind(&self) -> NorFlashErrorKind {
        self.kind
    }
}

impl From<NorFlashErrorKind> for FlashTestError {
    fn from(kind: NorFlashErrorKind) -> Self {
        Self { kind }
    }
}

struct FakeFlash {
    data: Vec<u8>,
    written: Vec<bool>,
}

impl FakeFlash {
    fn new(region: usize) -> Self {
        assert_eq!(region % ERASE_SIZE, 0);
        Self {
            data: vec![0xFF; region],
            written: vec![false; region / WRITE_SIZE],
        }
    }
}

impl ReadNorFlash for FakeFlash {
    const READ_SIZE: usize = 1;

    async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        let off = offset as usize;
        if off + bytes.len() > self.data.len() {
            return Err(NorFlashErrorKind::OutOfBounds.into());
        }
        bytes.copy_from_slice(&self.data[off..off + bytes.len()]);
        Ok(())
    }

    fn capacity(&self) -> usize {
        self.data.len()
    }
}

impl NorFlash for FakeFlash {
    const WRITE_SIZE: usize = WRITE_SIZE;
    const ERASE_SIZE: usize = ERASE_SIZE;

    async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        let f = from as usize;
        let t = to as usize;
        if !f.is_multiple_of(ERASE_SIZE)
            || !t.is_multiple_of(ERASE_SIZE)
            || t <= f
            || t > self.data.len()
        {
            return Err(NorFlashErrorKind::NotAligned.into());
        }
        self.data[f..t].fill(0xFF);
        for word in self
            .written
            .iter_mut()
            .skip(f / WRITE_SIZE)
            .take((t - f) / WRITE_SIZE)
        {
            *word = false;
        }
        Ok(())
    }

    async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        let off = offset as usize;
        if !off.is_multiple_of(WRITE_SIZE)
            || !bytes.len().is_multiple_of(WRITE_SIZE)
            || off + bytes.len() > self.data.len()
        {
            return Err(NorFlashErrorKind::NotAligned.into());
        }
        for (i, &b) in bytes.iter().enumerate() {
            let word = (off + i) / WRITE_SIZE;
            if self.written[word] {
                return Err(FlashTestError {
                    kind: NorFlashErrorKind::Other,
                });
            }
            let old = self.data[off + i];
            if old | b != old {
                return Err(FlashTestError {
                    kind: NorFlashErrorKind::Other,
                });
            }
        }
        for (i, &b) in bytes.iter().enumerate() {
            self.written[(off + i) / WRITE_SIZE] = true;
            self.data[off + i] = b;
        }
        Ok(())
    }
}

#[derive(Clone)]
struct RcFlash(Rc<RefCell<FakeFlash>>);

impl RcFlash {
    fn new() -> Self {
        Self(Rc::new(RefCell::new(FakeFlash::new(REGION_SIZE))))
    }
}

impl ErrorType for RcFlash {
    type Error = FlashTestError;
}

// The borrow is held across await so the fake serializes access to the shared
// device state; the tests are single-threaded, so there is no contention.
#[allow(clippy::await_holding_refcell_ref)]
impl ReadNorFlash for RcFlash {
    const READ_SIZE: usize = FakeFlash::READ_SIZE;

    async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.0.borrow_mut().read(offset, bytes).await
    }

    fn capacity(&self) -> usize {
        self.0.borrow().capacity()
    }
}

#[allow(clippy::await_holding_refcell_ref)]
impl NorFlash for RcFlash {
    const WRITE_SIZE: usize = FakeFlash::WRITE_SIZE;
    const ERASE_SIZE: usize = FakeFlash::ERASE_SIZE;

    async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        self.0.borrow_mut().erase(from, to).await
    }

    async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0.borrow_mut().write(offset, bytes).await
    }
}

fn flash_config() -> FlashConfig {
    FlashConfig::new(0, 512, 8, 32, 128, ERASE_SIZE).expect("valid test config")
}

fn new_store(flash: RcFlash) -> FlashKvStore<RcFlash> {
    FlashKvStore::new(flash, StorageConfig::default(), flash_config()).expect("valid store")
}

fn expect_invalid(store: Result<FlashKvStore<RcFlash>, StorageError>) -> StorageError {
    match store {
        Err(e) => e,
        Ok(_) => panic!("expected store construction to fail"),
    }
}

async fn open(store: &mut FlashKvStore<RcFlash>) {
    store.open().await.expect("open scans the region");
}

// Construction and geometry

#[test]
fn capacity_reserves_one_spare_sector() {
    let store = new_store(RcFlash::new());
    assert_eq!(store.capacity(), 7 * (512 - 8));
}

#[test]
fn new_rejects_sector_not_multiple_of_erase_size() {
    let err = expect_invalid(FlashKvStore::new(
        RcFlash::new(),
        StorageConfig::default(),
        FlashConfig {
            base_offset: 0,
            sector_size: 300,
            sector_count: 8,
            max_key_len: 32,
            max_value_len: 128,
        },
    ));
    assert!(matches!(err, StorageError::Internal(_)));
}

#[test]
fn new_rejects_region_beyond_capacity() {
    let err = expect_invalid(FlashKvStore::new(
        RcFlash::new(),
        StorageConfig::default(),
        FlashConfig::new(0, 512, 9, 32, 128, ERASE_SIZE).unwrap(),
    ));
    assert!(matches!(err, StorageError::Internal(_)));
}

#[test]
fn new_rejects_record_larger_than_sector() {
    let err = expect_invalid(FlashKvStore::new(
        RcFlash::new(),
        StorageConfig::default(),
        FlashConfig::new(0, 512, 8, 32, 500, ERASE_SIZE).unwrap(),
    ));
    assert!(matches!(err, StorageError::Internal(_)));
}

#[test]
fn operations_require_open() {
    let store = new_store(RcFlash::new());
    let err = block_on(store.get("ns", "k")).unwrap_err();
    assert!(matches!(err, StorageError::Internal(_)));
}

// Basic key-value operations

#[test]
fn put_and_get_roundtrip() {
    block_on(async {
        let mut store = new_store(RcFlash::new());
        open(&mut store).await;
        store.put("ns", "k", Bytes::from("hello")).await.unwrap();
        assert_eq!(
            store.get("ns", "k").await.unwrap(),
            Some(Bytes::from("hello"))
        );
    });
}

#[test]
fn put_overwrites_existing() {
    block_on(async {
        let mut store = new_store(RcFlash::new());
        open(&mut store).await;
        store.put("ns", "k", Bytes::from("v1")).await.unwrap();
        store.put("ns", "k", Bytes::from("v2")).await.unwrap();
        assert_eq!(store.get("ns", "k").await.unwrap(), Some(Bytes::from("v2")));
    });
}

#[test]
fn get_missing_returns_none() {
    block_on(async {
        let mut store = new_store(RcFlash::new());
        open(&mut store).await;
        assert_eq!(store.get("ns", "missing").await.unwrap(), None);
        assert!(!store.exists("ns", "missing").await.unwrap());
    });
}

#[test]
fn delete_removes_key() {
    block_on(async {
        let mut store = new_store(RcFlash::new());
        open(&mut store).await;
        store.put("ns", "k", Bytes::from("v")).await.unwrap();
        store.delete("ns", "k").await.unwrap();
        assert_eq!(store.get("ns", "k").await.unwrap(), None);
    });
}

#[test]
fn delete_missing_key_does_not_error() {
    block_on(async {
        let mut store = new_store(RcFlash::new());
        open(&mut store).await;
        store.delete("ns", "missing").await.unwrap();
    });
}

#[test]
fn list_keys_and_namespaces() {
    block_on(async {
        let mut store = new_store(RcFlash::new());
        open(&mut store).await;
        store.put("ns1", "a", Bytes::from("1")).await.unwrap();
        store.put("ns1", "b", Bytes::from("2")).await.unwrap();
        store.put("ns2", "k", Bytes::from("3")).await.unwrap();

        let mut keys = store.list_keys("ns1").await.unwrap();
        keys.sort();
        assert_eq!(keys, vec!["a", "b"]);
        assert_eq!(store.list_keys("ns2").await.unwrap(), vec!["k"]);

        let mut nss = store.list_namespaces().await.unwrap();
        nss.sort();
        assert_eq!(nss, vec!["ns1", "ns2"]);
    });
}

// Namespace lifecycle

#[test]
fn namespace_marker_survives_key_deletion() {
    block_on(async {
        let mut store = new_store(RcFlash::new());
        open(&mut store).await;
        store.create_namespace("empty").await.unwrap();
        store.put("empty", "k", Bytes::from("v")).await.unwrap();
        store.delete("empty", "k").await.unwrap();
        assert_eq!(store.list_namespaces().await.unwrap(), vec!["empty"]);
    });
}

#[test]
fn create_existing_namespace_errors() {
    block_on(async {
        let mut store = new_store(RcFlash::new());
        open(&mut store).await;
        store.create_namespace("ns").await.unwrap();
        let err = store.create_namespace("ns").await.unwrap_err();
        assert!(matches!(err, StorageError::NamespaceAlreadyExists(_)));
    });
}

#[test]
fn delete_namespace_removes_keys_and_marker() {
    block_on(async {
        let mut store = new_store(RcFlash::new());
        open(&mut store).await;
        store.put("ns", "k", Bytes::from("v")).await.unwrap();
        store.delete_namespace("ns").await.unwrap();
        assert_eq!(store.get("ns", "k").await.unwrap(), None);
        assert!(store.list_namespaces().await.unwrap().is_empty());
    });
}

#[test]
fn clear_namespace_keeps_namespace() {
    block_on(async {
        let mut store = new_store(RcFlash::new());
        open(&mut store).await;
        store.put("ns", "k", Bytes::from("v")).await.unwrap();
        store.clear_namespace("ns").await.unwrap();
        assert_eq!(store.get("ns", "k").await.unwrap(), None);
        assert_eq!(store.list_namespaces().await.unwrap(), vec!["ns"]);
        store.put("ns", "k2", Bytes::from("v")).await.unwrap();
        assert!(store.exists("ns", "k2").await.unwrap());
    });
}

// Size limits (Tier 2 orchestration bounds)

#[test]
fn rejects_key_over_limit() {
    block_on(async {
        let mut store = new_store(RcFlash::new());
        open(&mut store).await;
        let long_key = "x".repeat(33);
        let err = store
            .put("ns", &long_key, Bytes::from("v"))
            .await
            .unwrap_err();
        assert!(matches!(err, StorageError::Internal(_)));
    });
}

#[test]
fn rejects_value_over_limit() {
    block_on(async {
        let mut store = new_store(RcFlash::new());
        open(&mut store).await;
        let big = Bytes::from(vec![0u8; 129]);
        let err = store.put("ns", "k", big).await.unwrap_err();
        assert!(matches!(err, StorageError::QuotaExceeded(_)));
    });
}

#[test]
fn rejects_namespace_over_255_bytes() {
    block_on(async {
        let mut store = new_store(RcFlash::new());
        open(&mut store).await;
        let long_ns = "n".repeat(256);
        let err = store
            .put(&long_ns, "k", Bytes::from("v"))
            .await
            .unwrap_err();
        assert!(matches!(err, StorageError::Internal(_)));
    });
}

// auto_create_namespaces = false

#[test]
fn put_errors_on_missing_namespace_without_auto_create() {
    block_on(async {
        let mut store = FlashKvStore::new(
            RcFlash::new(),
            StorageConfig {
                auto_create_namespaces: false,
                ..Default::default()
            },
            flash_config(),
        )
        .unwrap();
        open(&mut store).await;
        let err = store
            .put("manual", "k", Bytes::from("v"))
            .await
            .unwrap_err();
        assert!(matches!(err, StorageError::NamespaceNotFound(_)));
    });
}

// Namespace prefix isolation

#[test]
fn namespace_prefix_isolates_storage() {
    block_on(async {
        let mut a = FlashKvStore::new(
            RcFlash::new(),
            StorageConfig::default().with_prefix("tenant_a"),
            flash_config(),
        )
        .unwrap();
        let mut b = FlashKvStore::new(
            RcFlash::new(),
            StorageConfig::default().with_prefix("tenant_b"),
            flash_config(),
        )
        .unwrap();
        open(&mut a).await;
        open(&mut b).await;

        a.put("ns", "k", Bytes::from("from_a")).await.unwrap();
        b.put("ns", "k", Bytes::from("from_b")).await.unwrap();

        assert_eq!(a.get("ns", "k").await.unwrap(), Some(Bytes::from("from_a")));
        assert_eq!(b.get("ns", "k").await.unwrap(), Some(Bytes::from("from_b")));
        assert_eq!(a.list_namespaces().await.unwrap(), vec!["ns"]);
    });
}

// Durability and rollover

#[test]
fn data_survives_reboot() {
    block_on(async {
        let flash = RcFlash::new();
        {
            let mut store = new_store(flash.clone());
            open(&mut store).await;
            store
                .put("ns", "k", Bytes::from("persisted"))
                .await
                .unwrap();
        }
        {
            let mut store = new_store(flash.clone());
            open(&mut store).await;
            assert_eq!(
                store.get("ns", "k").await.unwrap(),
                Some(Bytes::from("persisted"))
            );
        }
    });
}

#[test]
fn rolls_across_sectors_and_reads_back() {
    block_on(async {
        let flash = RcFlash::new();
        {
            let mut store = new_store(flash.clone());
            open(&mut store).await;
            for i in 0..40 {
                store
                    .put("ns", &format!("k{i}"), Bytes::from(vec![i as u8; 10]))
                    .await
                    .unwrap();
            }
        }
        {
            let mut store = new_store(flash.clone());
            open(&mut store).await;
            for i in 0..40 {
                assert_eq!(
                    store.get("ns", &format!("k{i}")).await.unwrap(),
                    Some(Bytes::from(vec![i as u8; 10])),
                    "key k{i} after reboot"
                );
            }
        }
    });
}

#[test]
fn compaction_reclaims_space_under_overwrite() {
    block_on(async {
        let flash = RcFlash::new();
        let mut store = new_store(flash.clone());
        open(&mut store).await;
        for i in 0..8 {
            store
                .put("ns", &format!("k{i}"), Bytes::from(vec![i as u8; 100]))
                .await
                .unwrap();
        }
        for round in 0..60 {
            store
                .put("ns", "k0", Bytes::from(vec![round as u8; 100]))
                .await
                .unwrap();
        }
        for i in 1..8 {
            assert_eq!(
                store.get("ns", &format!("k{i}")).await.unwrap(),
                Some(Bytes::from(vec![i as u8; 100])),
                "key k{i} after compaction"
            );
        }
        assert_eq!(
            store.get("ns", "k0").await.unwrap(),
            Some(Bytes::from(vec![59u8; 100]))
        );

        let mut reopened = new_store(flash.clone());
        open(&mut reopened).await;
        for i in 1..8 {
            assert_eq!(
                reopened.get("ns", &format!("k{i}")).await.unwrap(),
                Some(Bytes::from(vec![i as u8; 100]))
            );
        }
    });
}

#[test]
fn compaction_preserves_tombstones_and_markers() {
    block_on(async {
        let flash = RcFlash::new();
        let mut store = new_store(flash.clone());
        open(&mut store).await;
        store
            .put("ns", "dead", Bytes::from(vec![1u8; 100]))
            .await
            .unwrap();
        store
            .put("ns", "live", Bytes::from(vec![2u8; 100]))
            .await
            .unwrap();
        store.delete("ns", "dead").await.unwrap();
        for _ in 0..60 {
            store
                .put("ns", "churn", Bytes::from(vec![3u8; 100]))
                .await
                .unwrap();
        }
        assert_eq!(store.get("ns", "dead").await.unwrap(), None);
        assert_eq!(
            store.get("ns", "live").await.unwrap(),
            Some(Bytes::from(vec![2u8; 100]))
        );

        let mut reopened = new_store(flash.clone());
        open(&mut reopened).await;
        assert_eq!(reopened.get("ns", "dead").await.unwrap(), None);
        assert_eq!(
            reopened.get("ns", "live").await.unwrap(),
            Some(Bytes::from(vec![2u8; 100]))
        );
        let mut nss = reopened.list_namespaces().await.unwrap();
        nss.sort();
        assert_eq!(nss, vec!["ns"]);
    });
}

// Quota

#[test]
fn quota_exceeded_when_region_full_then_recoverable() {
    block_on(async {
        let mut store = new_store(RcFlash::new());
        open(&mut store).await;

        let mut err = None;
        for i in 0..64 {
            if let Err(e) = store
                .put("ns", &format!("k{i}"), Bytes::from(vec![i as u8; 100]))
                .await
            {
                err = Some((i, e));
                break;
            }
        }
        let (full_at, err) = err.expect("region must fill before 64 distinct keys");
        assert!(matches!(err, StorageError::QuotaExceeded(_)));
        assert!(
            full_at >= 29,
            "region should hold ~30 keys, filled at {full_at}"
        );

        for i in 0..full_at {
            assert_eq!(
                store.get("ns", &format!("k{i}")).await.unwrap(),
                Some(Bytes::from(vec![i as u8; 100])),
                "key k{i} after quota error"
            );
        }

        for i in 0..4 {
            store.delete("ns", &format!("k{i}")).await.unwrap();
        }
        store
            .put("ns", &format!("k{full_at}"), Bytes::from(vec![9u8; 100]))
            .await
            .expect("deleting keys must free compaction space");
        assert_eq!(
            store.get("ns", &format!("k{full_at}")).await.unwrap(),
            Some(Bytes::from(vec![9u8; 100]))
        );
    });
}

// Torn-write recovery

#[test]
fn open_truncates_torn_tail_record() {
    block_on(async {
        let flash = RcFlash::new();
        {
            let mut store = new_store(flash.clone());
            open(&mut store).await;
            store
                .put("ns", "a", Bytes::from(vec![1u8; 10]))
                .await
                .unwrap();
            store
                .put("ns", "b", Bytes::from(vec![2u8; 10]))
                .await
                .unwrap();
        }
        // Sector 0: 8-byte seq header, namespace marker at [8..24), "a" at
        // [24..52), "b" at [52..80). Corrupt "b"'s ns_len byte so its header
        // is invalid.
        {
            let mut fake = flash.0.borrow_mut();
            fake.data[52 + 1] = 0xFF;
        }
        let mut reopened = new_store(flash.clone());
        open(&mut reopened).await;
        assert_eq!(
            reopened.get("ns", "a").await.unwrap(),
            Some(Bytes::from(vec![1u8; 10]))
        );
        assert_eq!(reopened.get("ns", "b").await.unwrap(), None);
    });
}
