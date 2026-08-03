#![cfg(all(
    not(feature = "drbg"),
    any(feature = "os", feature = "wasm", feature = "custom")
))]

use saikuro_random::{u32, u64, uuid_v4};

#[test]
fn uuid_v4_sets_version_and_variant_bits() {
    let uuid = uuid_v4().expect("entropy available");
    assert_eq!(uuid.get_version_num(), 4);
    let bytes = uuid.as_bytes();
    assert_eq!(bytes[6] >> 4, 4);
    assert_eq!(bytes[8] & 0xc0, 0x80);
}

#[test]
fn generated_uuids_are_unique() {
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..64 {
        let uuid = uuid_v4().expect("entropy available");
        assert!(seen.insert(uuid), "duplicate uuid {uuid}");
    }
}

#[test]
fn u32_and_u64_draws_are_sane() {
    let mut words = std::collections::BTreeSet::new();
    for _ in 0..4 {
        words.insert(u32().expect("entropy available"));
    }
    assert!(
        words.len() >= 2,
        "four draws all colliding is statistically impossible"
    );

    let x = u64().expect("entropy available");
    assert!(x != 0 || u64().expect("entropy available") != 0);
}
