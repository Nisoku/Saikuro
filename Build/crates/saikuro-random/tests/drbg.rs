#![cfg(feature = "drbg")]

use chacha20::cipher::{KeyIvInit, StreamCipher};
use chacha20::XChaCha20;
use saikuro_random::{fill, is_seeded, seed_from_slice, Drbg, Error};

const SEED_LEN: usize = 56;
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;
const BLOCK_LEN: usize = 64;

const SEED_ONE: [u8; SEED_LEN] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
    0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20,
    0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f, 0x30,
    0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38,
];

#[test]
fn same_seed_is_reproducible() {
    let mut a = Drbg::from_seed(&SEED_ONE).expect("valid seed");
    let mut b = Drbg::from_seed(&SEED_ONE).expect("valid seed");
    let mut buf_a = [0u8; 128];
    let mut buf_b = [0u8; 128];
    a.fill(&mut buf_a).expect("fill ok");
    b.fill(&mut buf_b).expect("fill ok");
    assert_eq!(buf_a, buf_b);
}

#[test]
fn different_seeds_diverge() {
    let mut a = Drbg::from_seed(&SEED_ONE).expect("valid seed");
    let mut b = Drbg::from_seed(&[0xee; SEED_LEN]).expect("valid seed");
    let mut buf_a = [0u8; 64];
    let mut buf_b = [0u8; 64];
    a.fill(&mut buf_a).expect("fill ok");
    b.fill(&mut buf_b).expect("fill ok");
    assert_ne!(buf_a, buf_b);
}

#[test]
fn drbg_matches_reference_stream() {
    let mut drbg = Drbg::from_seed(&SEED_ONE).expect("valid seed");
    let mut buf = [0u8; 128];
    drbg.fill(&mut buf).expect("fill ok");

    let key = &SEED_ONE[..KEY_LEN];
    let nonce = &SEED_ONE[KEY_LEN..SEED_LEN];
    let mut cipher = XChaCha20::new_from_slices(key, nonce).expect("valid lengths");
    let mut block_zero = [0u8; BLOCK_LEN];
    cipher.apply_keystream(&mut block_zero);
    assert_eq!(
        buf[..BLOCK_LEN],
        block_zero,
        "seek(0) must equal the first keystream block"
    );
    let mut block_one = [0u8; BLOCK_LEN];
    cipher.apply_keystream(&mut block_one);
    assert_eq!(
        buf[BLOCK_LEN..],
        block_one,
        "sequential blocks must be contiguous"
    );
}

#[test]
fn short_seed_is_rejected() {
    assert_eq!(Drbg::from_seed(&[0u8; 8]), Err(Error::InvalidSeed));
}

#[test]
fn global_stream_matches_a_seeded_local_drbg_and_advances() {
    seed_from_slice(&SEED_ONE).expect("valid seed");
    assert!(is_seeded());

    let mut first = [0u8; 32];
    fill(&mut first).expect("seeded fill ok");
    let mut second = [0u8; 32];
    fill(&mut second).expect("seeded fill ok");

    let mut drbg = Drbg::from_seed(&SEED_ONE).expect("valid seed");
    let mut expected = [0u8; 128];
    drbg.fill(&mut expected).expect("fill ok");
    assert_eq!(
        &first[..],
        &expected[..32],
        "first draw must match the head of the stream"
    );
    assert_eq!(
        &second[..],
        &expected[64..96],
        "second draw must continue the stream at the next block"
    );
}

#[test]
fn fill_uninit_initializes_every_byte() {
    let mut drbg = Drbg::from_seed(&SEED_ONE).expect("valid seed");
    let mut buf = [core::mem::MaybeUninit::<u8>::uninit(); 16];
    drbg.fill_uninit(&mut buf).expect("fill ok");
    let buf = buf.map(|slot| {
        // SAFETY: fill_uninit initialized every slot on Ok.
        unsafe { slot.assume_init() }
    });
    let mut expected = [0u8; 16];
    let mut probe = Drbg::from_seed(&SEED_ONE).expect("valid seed");
    probe.fill(&mut expected).expect("fill ok");
    assert_eq!(&buf[..], &expected[..]);
}
