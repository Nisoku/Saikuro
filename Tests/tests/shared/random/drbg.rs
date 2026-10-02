use crate::shared_test;
use crate::TestSuite;
use saikuro_random::{Drbg, SEED_LEN};

// The counter-mode ChaCha20 DRBG. Tests use fresh `Drbg` instances

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "random::drbg_deterministic_across_instances",
        drbg_deterministic_across_instances,
    );
    shared_test!(
        suite,
        "random::drbg_chunked_fill_matches_oneshot",
        drbg_chunked_fill_matches_oneshot,
    );
    shared_test!(
        suite,
        "random::drbg_rejects_seed_below_seed_len",
        drbg_rejects_seed_below_seed_len,
    );
    shared_test!(
        suite,
        "random::drbg_accepts_exactly_seed_len",
        drbg_accepts_exactly_seed_len,
    );
    shared_test!(
        suite,
        "random::drbg_ignores_extra_seed_bytes",
        drbg_ignores_extra_seed_bytes,
    );
    shared_test!(
        suite,
        "random::drbg_empty_fill_is_noop",
        drbg_empty_fill_is_noop,
    );
    shared_test!(
        suite,
        "random::drbg_fill_uninit_initializes_all",
        drbg_fill_uninit_initializes_all,
    );
    shared_test!(
        suite,
        "random::drbg_argument_order_affects_stream",
        drbg_argument_order_affects_stream,
    );
}

fn deterministic_stream(seed: &[u8], len: usize) -> Result<crate::Vec<u8>, &'static str> {
    let mut drbg = Drbg::from_seed(seed).map_err(|_| "seed must be accepted")?;
    let mut out = crate::vec![0u8; len];
    drbg.fill(&mut out).map_err(|_| "fill must succeed")?;
    Ok(out)
}

fn drbg_deterministic_across_instances() -> Result<(), &'static str> {
    let seed = [0x42u8; SEED_LEN];
    let a = deterministic_stream(&seed, 64)?;
    let b = deterministic_stream(&seed, 64)?;
    crate::check_test!(a == b, "same seed must produce the same keystream");
    let variant = deterministic_stream(&seed, 64)?;
    crate::check_test!(variant == a, "keystream must be stable across reruns");
    Ok(())
}

fn drbg_chunked_fill_matches_oneshot() -> Result<(), &'static str> {
    let seed = [0xA5u8; SEED_LEN];
    let oneshot = deterministic_stream(&seed, 69)?;

    // The keystream advances in whole 64-byte blocks, so continuity is only
    // guaranteed when every fill ends on a block boundary (or a partial fill
    // follows a complete one).
    let mut drbg = Drbg::from_seed(&seed).map_err(|_| "seed must be accepted")?;
    let mut chunked = crate::Vec::new();
    for &size in &[64usize, 5, 64, 64] {
        let mut buf = crate::vec![0u8; size];
        drbg.fill(&mut buf).map_err(|_| "fill must succeed")?;
        chunked.extend_from_slice(&buf);
    }
    crate::check_test!(
        chunked[..69] == oneshot[..],
        "block-aligned partial fills must produce the same stream prefix"
    );
    Ok(())
}

fn drbg_rejects_seed_below_seed_len() -> Result<(), &'static str> {
    crate::check_test!(
        Drbg::from_seed(&[0u8; SEED_LEN - 1]).is_err(),
        "a seed one byte short must be rejected"
    );
    crate::check_test!(
        Drbg::from_seed(&[]).is_err(),
        "an empty seed must be rejected"
    );
    Ok(())
}

fn drbg_accepts_exactly_seed_len() -> Result<(), &'static str> {
    crate::check_test!(
        Drbg::from_seed(&[7u8; SEED_LEN]).is_ok(),
        "a seed of exactly SEED_LEN bytes must be accepted"
    );
    Ok(())
}

fn drbg_ignores_extra_seed_bytes() -> Result<(), &'static str> {
    let base = deterministic_stream(&[7u8; SEED_LEN], 32)?;
    let padded_seed: crate::Vec<u8> = {
        let mut v = crate::vec![7u8; SEED_LEN];
        v.extend_from_slice(&[9u8; 24]);
        v
    };
    let padded = deterministic_stream(&padded_seed, 32)?;
    crate::check_test!(base == padded, "bytes past SEED_LEN must be ignored");
    Ok(())
}

fn drbg_empty_fill_is_noop() -> Result<(), &'static str> {
    let seed = [0x11u8; SEED_LEN];
    let mut plain = Drbg::from_seed(&seed).map_err(|_| "seed must be accepted")?;
    let mut plain_out = [0u8; 16];
    plain
        .fill(&mut plain_out)
        .map_err(|_| "fill must succeed")?;

    let mut capped = Drbg::from_seed(&seed).map_err(|_| "seed must be accepted")?;
    let mut empty = [];
    capped
        .fill(&mut empty)
        .map_err(|_| "empty fill must succeed")?;
    let mut capped_out = [0u8; 16];
    capped
        .fill(&mut capped_out)
        .map_err(|_| "fill must succeed")?;

    crate::check_test!(
        plain_out == capped_out,
        "an empty fill must not advance the keystream"
    );
    Ok(())
}

fn drbg_fill_uninit_initializes_all() -> Result<(), &'static str> {
    let seed = [0xE7u8; SEED_LEN];
    let mut drbg = Drbg::from_seed(&seed).map_err(|_| "seed must be accepted")?;
    let mut uninit = [core::mem::MaybeUninit::<u8>::uninit(); 13];
    drbg.fill_uninit(&mut uninit)
        .map_err(|_| "fill_uninit must succeed")?;
    // SAFETY: fill_uninit promises to initialize every byte on Ok.
    let inited: [u8; 13] = uninit.map(|u| unsafe { u.assume_init() });
    let expected = deterministic_stream(&seed, 13)?;
    crate::check_test!(
        inited[..] == expected[..],
        "fill_uninit must match the regular fill output"
    );
    Ok(())
}

fn drbg_argument_order_affects_stream() -> Result<(), &'static str> {
    let seed = [0x99u8; SEED_LEN];
    let mut a = Drbg::from_seed(&seed).map_err(|_| "seed must be accepted")?;
    let mut first = [0u8; 8];
    let mut second = [0u8; 8];
    a.fill(&mut first).map_err(|_| "fill must succeed")?;
    a.fill(&mut second).map_err(|_| "fill must succeed")?;
    let mut combined = [0u8; 16];
    combined[..8].copy_from_slice(&first);
    combined[8..].copy_from_slice(&second);

    // A sub-block fill advances the block counter, so 8+8 is *not* byte
    // equivalent to a single 16-byte fill: the second 8-byte fill begins at
    // the next 64-byte block, discarding the tail of the first.
    let mut b = Drbg::from_seed(&seed).map_err(|_| "seed must be accepted")?;
    let mut oneshot = [0u8; 16];
    b.fill(&mut oneshot).map_err(|_| "fill must succeed")?;
    crate::check_test!(
        combined != oneshot,
        "sub-block fills must advance a whole block, not a byte range"
    );
    Ok(())
}
