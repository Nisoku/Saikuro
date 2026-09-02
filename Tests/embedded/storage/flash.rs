//! Integration tests for the flash-backed key-value store on the embedded
//! engine. Run inside the Cortex-M / RISC-V QEMU binaries with the `flash`
//! feature enabled.

use alloc::format;
use alloc::rc::Rc;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use saikuro_storage::{Bytes, FlashConfig, FlashKvStore, SaikuroError, StorageConfig};
use saikuro_tests::block_on;

mod mock {
    use alloc::rc::Rc;
    use alloc::vec::Vec;
    use core::cell::RefCell;

    use embedded_storage_async::nor_flash::{ErrorType as EmbeddedStorageAsync, NorFlash, ReadNorFlash};

    pub const ERASE_SIZE: usize = 256;
    pub const WRITE_SIZE: usize = 4;
    pub const SECTORS: usize = 8;
    pub const CAPACITY: usize = ERASE_SIZE * SECTORS;

    /// A shared, in-memory mock of a `NorFlash` device.
    ///
    /// It models the two physical constraints of real NOR flash:
    /// - a word can only be written once between erases, and
    /// - a write may only clear bits (set them to 0), never set them to 1.
    #[derive(Clone)]
    pub struct RcFlash {
        pub cells: Rc<RefCell<Vec<u8>>>,
        pub written: Rc<RefCell<Vec<usize>>>,
        pub erase_size: usize,
        pub write_size: usize,
        pub capacity: usize,
    }

    impl RcFlash {
        pub fn new() -> Self {
            let mut cells = Vec::with_capacity(CAPACITY);
            cells.resize(CAPACITY, 0xFF);
            Self {
                cells: Rc::new(RefCell::new(cells)),
                written: Rc::new(RefCell::new(Vec::new())),
                erase_size: ERASE_SIZE,
                write_size: WRITE_SIZE,
                capacity: CAPACITY,
            }
        }

        /// Overwrite the final erase sector with zeroes, simulating bit flips in
        /// an otherwise-unused tail/spare region after a crash.
        pub fn corrupt_tail(&self) {
            let mut cells = self.cells.borrow_mut();
            let start = self.capacity - self.erase_size;
            for b in cells.iter_mut().skip(start) {
                *b = 0x00;
            }
        }
    }

    impl EmbeddedStorageAsync for RcFlash {
        type Error = core::convert::Infallible;
    }

    impl ReadNorFlash for RcFlash {
        const READ_SIZE: usize = WRITE_SIZE;

        async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
            let cells = self.cells.borrow();
            bytes.copy_from_slice(&cells[offset as usize..offset as usize + bytes.len()]);
            Ok(())
        }

        fn capacity(&self) -> usize {
            self.capacity
        }
    }

    impl NorFlash for RcFlash {
        const WRITE_SIZE: usize = WRITE_SIZE;
        const ERASE_SIZE: usize = ERASE_SIZE;

        async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
            assert_eq!(
                offset as usize % WRITE_SIZE,
                0,
                "write must be word-aligned"
            );
            assert_eq!(
                bytes.len() % WRITE_SIZE,
                0,
                "write length must be a word multiple"
            );
            let mut cells = self.cells.borrow_mut();
            let mut written = self.written.borrow_mut();
            for (i, &b) in bytes.iter().enumerate() {
                let idx = offset as usize + i;
                let old = cells[idx];
                assert!(
                    old | b == old,
                    "NOR flash cannot set bits: wrote {b:#04x} over {old:#04x}"
                );
                assert!(
                    !written.contains(&idx),
                    "NOR flash cannot rewrite a word without an erase"
                );
                cells[idx] = b;
                written.push(idx);
            }
            Ok(())
        }

        async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
            let mut cells = self.cells.borrow_mut();
            for b in cells.iter_mut().take(to as usize).skip(from as usize) {
                *b = 0xFF;
            }
            let mut written = self.written.borrow_mut();
            written.retain(|w| *w < from as usize || *w >= to as usize);
            Ok(())
        }
    }
}

use mock::{RcFlash, ERASE_SIZE, SECTORS};

fn flash_config() -> FlashConfig {
    // sector_size == ERASE_SIZE so the region (sector_size * SECTORS) exactly
    // fills the mock device capacity (ERASE_SIZE * SECTORS).
    FlashConfig::new(0, ERASE_SIZE, SECTORS, 32, 128, ERASE_SIZE).expect("valid flash config")
}

fn config(auto_create: bool) -> StorageConfig {
    StorageConfig {
        auto_create_namespaces: auto_create,
        ..Default::default()
    }
}

pub fn register(suite: &mut saikuro_tests::TestSuite) {
    suite.register("storage::flash_config_validates_geometry", flash_config_validates_geometry);
    suite.register("storage::flash_store_rejects_item_over_64kib", store_rejects_item_over_64kib);
    suite.register("storage::flash_operations_do_not_require_open", operations_do_not_require_open);
    suite.register("storage::flash_get_put_roundtrip", get_put_roundtrip);
    suite.register(
        "storage::flash_rejects_value_exceeding_max_value_len",
        rejects_value_exceeding_max_value_len,
    );
    suite.register(
        "storage::flash_rejects_key_exceeding_max_key_len",
        rejects_key_exceeding_max_key_len,
    );
    suite.register(
        "storage::flash_rejects_namespace_exceeding_255_bytes",
        rejects_namespace_exceeding_255_bytes,
    );
    suite.register("storage::flash_accepts_item_at_exact_limits", accepts_item_at_exact_limits);
    suite.register(
        "storage::flash_auto_create_creates_namespace_implicitly",
        auto_create_creates_namespace_implicitly,
    );
    suite.register(
        "storage::flash_auto_create_disabled_returns_not_found",
        auto_create_disabled_returns_not_found,
    );
    suite.register("storage::flash_prefix_isolation", prefix_isolation);
    suite.register("storage::flash_namespaces_are_independent", namespaces_are_independent);
    suite.register("storage::flash_durability_survives_reboot", durability_survives_reboot);
    suite.register("storage::flash_mount_tolerates_tail_corruption", mount_tolerates_tail_corruption);
    suite.register(
        "storage::flash_compaction_rolls_over_without_data_loss",
        compaction_rolls_over_without_data_loss,
    );
    suite.register(
        "storage::flash_quota_exceeded_when_region_full_then_recoverable",
        quota_exceeded_when_region_full_then_recoverable,
    );
    suite.register("storage::flash_delete_is_idempotent", delete_is_idempotent);
    suite.register("storage::flash_list_keys_returns_only_live", list_keys_returns_only_live);
    suite.register("storage::flash_list_namespaces_includes_all", list_namespaces_includes_all);
    suite.register(
        "storage::flash_namespace_marker_records_existence",
        namespace_marker_records_existence,
    );
    suite.register(
        "storage::flash_tombstone_distinguishes_present_from_absent",
        tombstone_distinguishes_present_from_absent,
    );
    suite.register("storage::flash_clear_namespace_keeps_namespace", clear_namespace_keeps_namespace);
    suite.register("storage::flash_delete_namespace_removes_all", delete_namespace_removes_all);
    suite.register("storage::flash_deleting_unknown_namespace_is_ok", deleting_unknown_namespace_is_ok);
}

fn flash_config_validates_geometry() -> Result<(), &'static str> {
    // base_offset not aligned to the erase size
    assert!(FlashConfig::new(1, 512, SECTORS, 32, 128, ERASE_SIZE).is_err());
    // fewer than two sectors
    assert!(FlashConfig::new(0, 512, 1, 32, 128, ERASE_SIZE).is_err());
    // sector size not a multiple of the erase size
    assert!(FlashConfig::new(0, 511, SECTORS, 32, 128, ERASE_SIZE).is_err());
    // key length above the u16 ceiling
    assert!(FlashConfig::new(0, 512, SECTORS, 70000, 128, ERASE_SIZE).is_err());
    // zero value length
    assert!(FlashConfig::new(0, 512, SECTORS, 32, 0, ERASE_SIZE).is_err());
    // erase size of 1 divides 512, so this is valid
    assert!(FlashConfig::new(0, 512, SECTORS, 32, 128, 1).is_ok());
    Ok(())
}

fn store_rejects_item_over_64kib() -> Result<(), &'static str> {
    // At the FlashConfig level a 70 KiB value is accepted, but sequential-storage
    // cannot represent an item larger than 64 KiB, so the store must reject it.
    let oversized = FlashConfig::new(0, 512, SECTORS, 32, 70000, ERASE_SIZE).unwrap();
    assert!(FlashKvStore::new(RcFlash::new(), config(true), oversized).is_err());
    Ok(())
}

fn operations_do_not_require_open() -> Result<(), &'static str> {
    block_on(async {
        let store = FlashKvStore::new(RcFlash::new(), config(true), flash_config()).unwrap();
        store
            .put("ns", "a", Bytes::from_static(b"1"))
            .await
            .unwrap();
        assert_eq!(
            store.get("ns", "a").await.unwrap(),
            Some(Bytes::from_static(b"1"))
        );
    });
    Ok(())
}

fn get_put_roundtrip() -> Result<(), &'static str> {
    block_on(async {
        let store = FlashKvStore::new(RcFlash::new(), config(true), flash_config()).unwrap();
        store
            .put("ns", "key", Bytes::from_static(b"value"))
            .await
            .unwrap();
        assert_eq!(
            store.get("ns", "key").await.unwrap(),
            Some(Bytes::from_static(b"value"))
        );
        assert_eq!(store.get("ns", "missing").await.unwrap(), None);
    });
    Ok(())
}

fn rejects_value_exceeding_max_value_len() -> Result<(), &'static str> {
    block_on(async {
        let store = FlashKvStore::new(RcFlash::new(), config(true), flash_config()).unwrap();
        let too_big = Bytes::from(vec![0xABu8; 129]);
        let e = store.put("ns", "k", too_big).await.unwrap_err();
        assert!(matches!(e, SaikuroError::QuotaExceeded { .. }));
    });
    Ok(())
}

fn rejects_key_exceeding_max_key_len() -> Result<(), &'static str> {
    block_on(async {
        let store = FlashKvStore::new(RcFlash::new(), config(true), flash_config()).unwrap();
        let long_key = "k".repeat(33);
        assert!(store
            .put("ns", &long_key, Bytes::from_static(b"v"))
            .await
            .is_err());
    });
    Ok(())
}

fn rejects_namespace_exceeding_255_bytes() -> Result<(), &'static str> {
    block_on(async {
        let store = FlashKvStore::new(RcFlash::new(), config(true), flash_config()).unwrap();
        let long_ns = "n".repeat(256);
        assert!(store
            .put(&long_ns, "k", Bytes::from_static(b"v"))
            .await
            .is_err());
        assert!(store.create_namespace(&long_ns).await.is_err());
    });
    Ok(())
}

fn accepts_item_at_exact_limits() -> Result<(), &'static str> {
    block_on(async {
        let store = FlashKvStore::new(RcFlash::new(), config(true), flash_config()).unwrap();
        let key = "k".repeat(32);
        let value = Bytes::from(vec![0xCDu8; 128]);
        store.put("ns", &key, value.clone()).await.unwrap();
        assert_eq!(store.get("ns", &key).await.unwrap(), Some(value));
    });
    Ok(())
}

fn auto_create_creates_namespace_implicitly() -> Result<(), &'static str> {
    block_on(async {
        let store = FlashKvStore::new(RcFlash::new(), config(true), flash_config()).unwrap();
        store
            .put("auto", "k", Bytes::from_static(b"v"))
            .await
            .unwrap();
        assert!(store.exists("auto", "k").await.unwrap());
        let namespaces = store.list_namespaces().await.unwrap();
        assert!(namespaces.contains(&"auto".to_string()));
    });
    Ok(())
}

fn auto_create_disabled_returns_not_found() -> Result<(), &'static str> {
    block_on(async {
        let store = FlashKvStore::new(RcFlash::new(), config(false), flash_config()).unwrap();
        let v = Bytes::from_static(b"v");
        let e = store.put("absent", "k", v.clone()).await.unwrap_err();
        assert!(matches!(e, SaikuroError::NamespaceNotFound(_)));
        let e = store.get("absent", "k").await.unwrap_err();
        assert!(matches!(e, SaikuroError::NamespaceNotFound(_)));
    });
    Ok(())
}

fn prefix_isolation() -> Result<(), &'static str> {
    block_on(async {
        let flash = RcFlash::new();
        let prod = FlashKvStore::new(
            flash.clone(),
            StorageConfig {
                namespace_prefix: Some("prod".to_string()),
                auto_create_namespaces: true,
                ..Default::default()
            },
            flash_config(),
        )
        .unwrap();
        let v = Bytes::from_static(b"v");
        prod.put("ns", "k", v.clone()).await.unwrap();

        let dev = FlashKvStore::new(
            flash.clone(),
            StorageConfig {
                namespace_prefix: Some("dev".to_string()),
                auto_create_namespaces: true,
                ..Default::default()
            },
            flash_config(),
        )
        .unwrap();
        assert_eq!(dev.get("ns", "k").await.unwrap(), None);
        assert_eq!(prod.get("ns", "k").await.unwrap(), Some(v));
    });
    Ok(())
}

fn namespaces_are_independent() -> Result<(), &'static str> {
    block_on(async {
        let store = FlashKvStore::new(RcFlash::new(), config(true), flash_config()).unwrap();
        let va = Bytes::from_static(b"a");
        let vb = Bytes::from_static(b"b");
        store.put("a", "k", va.clone()).await.unwrap();
        store.put("b", "k", vb.clone()).await.unwrap();
        assert_eq!(store.get("a", "k").await.unwrap(), Some(va));
        assert_eq!(store.get("b", "k").await.unwrap(), Some(vb));
    });
    Ok(())
}

fn durability_survives_reboot() -> Result<(), &'static str> {
    block_on(async {
        let flash = RcFlash::new();
        {
            let store = FlashKvStore::new(flash.clone(), config(true), flash_config()).unwrap();
            store.create_namespace("user").await.unwrap();
            store
                .put("user", "name", Bytes::from_static(b"neo"))
                .await
                .unwrap();
            store
                .put("user", "role", Bytes::from_static(b"admin"))
                .await
                .unwrap();
        }
        // New instance mounted over the same flash: committed data survives.
        let store = FlashKvStore::new(flash.clone(), config(true), flash_config()).unwrap();
        assert!(store.exists("user", "name").await.unwrap());
        assert_eq!(
            store.get("user", "name").await.unwrap(),
            Some(Bytes::from_static(b"neo"))
        );
        assert_eq!(
            store.get("user", "role").await.unwrap(),
            Some(Bytes::from_static(b"admin"))
        );
    });
    Ok(())
}

fn mount_tolerates_tail_corruption() -> Result<(), &'static str> {
    block_on(async {
        let flash = RcFlash::new();
        let store = FlashKvStore::new(flash.clone(), config(true), flash_config()).unwrap();
        store
            .put("ns", "a", Bytes::from_static(b"1"))
            .await
            .unwrap();
        drop(store);
        // Simulate bit flips in an otherwise-unused tail region after a crash.
        flash.corrupt_tail();
        let store = FlashKvStore::new(flash, config(true), flash_config()).unwrap();
        assert_eq!(
            store.get("ns", "a").await.unwrap(),
            Some(Bytes::from_static(b"1"))
        );
    });
    Ok(())
}

fn compaction_rolls_over_without_data_loss() -> Result<(), &'static str> {
    block_on(async {
        let store = FlashKvStore::new(RcFlash::new(), config(true), flash_config()).unwrap();
        let mut written = Vec::new();
        for i in 0..200u32 {
            let key = format!("k{i}");
            let value = Bytes::from(vec![(i & 0xFF) as u8; 10]);
            match store.put("ns", &key, value.clone()).await {
                Ok(()) => written.push((key, value)),
                Err(e) if matches!(e, SaikuroError::QuotaExceeded { .. }) => break,
                Err(e) => panic!("unexpected error: {e:?}"),
            }
        }
        assert!(
            written.len() >= 20,
            "expected rollover to absorb at least 20 items, got {}",
            written.len()
        );
        for (key, value) in &written {
            assert_eq!(store.get("ns", key).await.unwrap(), Some(value.clone()));
        }
    });
    Ok(())
}

fn quota_exceeded_when_region_full_then_recoverable() -> Result<(), &'static str> {
    block_on(async {
        let store = FlashKvStore::new(RcFlash::new(), config(true), flash_config()).unwrap();
        let value = Bytes::from(vec![0xABu8; 29]);
        let mut count = 0;
        loop {
            let key = format!("k{count}");
            match store.put("ns", &key, value.clone()).await {
                Ok(()) => count += 1,
                Err(e) if matches!(e, SaikuroError::QuotaExceeded { .. }) => break,
                Err(e) => panic!("unexpected error: {e:?}"),
            }
            assert!(count < 1000, "never hit the quota");
        }
        assert!(
            count >= 5,
            "expected a handful of items before full, got {count}"
        );
        // Freeing space makes the region writable again.
        for i in 0..count / 2 {
            store.delete("ns", &format!("k{i}")).await.unwrap();
        }
        store.put("ns", "extra", value.clone()).await.unwrap();
        assert_eq!(store.get("ns", "extra").await.unwrap(), Some(value));
    });
    Ok(())
}

fn delete_is_idempotent() -> Result<(), &'static str> {
    block_on(async {
        let store = FlashKvStore::new(RcFlash::new(), config(true), flash_config()).unwrap();
        let v = Bytes::from_static(b"v");
        store.put("ns", "k", v).await.unwrap();
        store.delete("ns", "k").await.unwrap();
        store.delete("ns", "k").await.unwrap();
        assert_eq!(store.get("ns", "k").await.unwrap(), None);
    });
    Ok(())
}

fn list_keys_returns_only_live() -> Result<(), &'static str> {
    block_on(async {
        let store = FlashKvStore::new(RcFlash::new(), config(true), flash_config()).unwrap();
        let v = Bytes::from_static(b"v");
        store.put("ns", "a", v.clone()).await.unwrap();
        store.put("ns", "b", v.clone()).await.unwrap();
        store.delete("ns", "a").await.unwrap();
        let mut keys = store.list_keys("ns").await.unwrap();
        keys.sort();
        assert_eq!(keys, vec!["b".to_string()]);
    });
    Ok(())
}

fn list_namespaces_includes_all() -> Result<(), &'static str> {
    block_on(async {
        let store = FlashKvStore::new(RcFlash::new(), config(true), flash_config()).unwrap();
        store.create_namespace("a").await.unwrap();
        store.create_namespace("b").await.unwrap();
        let mut ns = store.list_namespaces().await.unwrap();
        ns.sort();
        assert_eq!(ns, vec!["a".to_string(), "b".to_string()]);
    });
    Ok(())
}

fn namespace_marker_records_existence() -> Result<(), &'static str> {
    block_on(async {
        let store = FlashKvStore::new(RcFlash::new(), config(true), flash_config()).unwrap();
        store.create_namespace("ns").await.unwrap();
        let mut ns = store.list_namespaces().await.unwrap();
        ns.sort();
        assert_eq!(ns, vec!["ns".to_string()]);
        let e = store.create_namespace("ns").await.unwrap_err();
        assert!(matches!(e, SaikuroError::NamespaceAlreadyExists(_)));
    });
    Ok(())
}

fn tombstone_distinguishes_present_from_absent() -> Result<(), &'static str> {
    block_on(async {
        let store = FlashKvStore::new(RcFlash::new(), config(true), flash_config()).unwrap();
        let v = Bytes::from_static(b"v");
        store.put("ns", "k", v).await.unwrap();
        assert!(store.exists("ns", "k").await.unwrap());
        store.delete("ns", "k").await.unwrap();
        assert!(!store.exists("ns", "k").await.unwrap());
        assert_eq!(store.get("ns", "k").await.unwrap(), None);
        assert!(!store.exists("ns", "never").await.unwrap());
        assert_eq!(store.get("ns", "never").await.unwrap(), None);
    });
    Ok(())
}

fn clear_namespace_keeps_namespace() -> Result<(), &'static str> {
    block_on(async {
        let store = FlashKvStore::new(RcFlash::new(), config(true), flash_config()).unwrap();
        let v = Bytes::from_static(b"v");
        store.create_namespace("ns").await.unwrap();
        store.put("ns", "a", v).await.unwrap();
        store.clear_namespace("ns").await.unwrap();
        assert_eq!(store.get("ns", "a").await.unwrap(), None);
        assert!(!store.exists("ns", "a").await.unwrap());
        let ns = store.list_namespaces().await.unwrap();
        assert!(ns.contains(&"ns".to_string()));
    });
    Ok(())
}

fn delete_namespace_removes_all() -> Result<(), &'static str> {
    block_on(async {
        let store = FlashKvStore::new(RcFlash::new(), config(true), flash_config()).unwrap();
        let v = Bytes::from_static(b"v");
        store.create_namespace("ns").await.unwrap();
        store.put("ns", "a", v.clone()).await.unwrap();
        store.put("ns", "b", v).await.unwrap();
        store.delete_namespace("ns").await.unwrap();
        // The namespace is gone entirely.
        let e = store.get("ns", "a").await.unwrap_err();
        assert!(matches!(e, SaikuroError::NamespaceNotFound(_)));
        let ns = store.list_namespaces().await.unwrap();
        assert!(!ns.contains(&"ns".to_string()));
    });
    Ok(())
}

fn deleting_unknown_namespace_is_ok() -> Result<(), &'static str> {
    block_on(async {
        let store = FlashKvStore::new(RcFlash::new(), config(true), flash_config()).unwrap();
        store.delete_namespace("ghost").await.unwrap();
    });
    Ok(())
}