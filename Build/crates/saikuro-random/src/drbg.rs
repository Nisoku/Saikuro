//! Deterministic ChaCha20 DRBG backend.
//!
//! This is a counter-mode DRBG over the RFC 8439 ChaCha20 stream cipher. Block
//! `n` of the keystream is `ChaCha20(key, nonce)` seeked to byte `n * 64`, so
//! the whole stream is just a function of the 56-byte seed (32-byte key +
//! 24-byte XChaCha20 nonce). Same seed in, same bytes out: that's the whole
//! point, since it lets tests be deterministic.
//!
//! On MCUs with no entropy source (RP2040, say) the binary seeds the global
//! state from whatever weak entropy the hardware has lying around (ROSC jitter)
//! and every [`crate::fill`] call pulls from that.

// We use portable-atomic rather than core::sync::atomic because some of the MCU
// targets don't have the atomics we need: riscv32imc and thumbv6m have no
// native atomics at all, and riscv32imac has no 64-bit ones. portable-atomic
// uses native instructions when they're there and falls back to
// critical-section otherwise, so this module keeps compiling everywhere.
use portable_atomic::{AtomicBool, AtomicU64, Ordering};

use chacha20::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
use chacha20::XChaCha20;

/// ChaCha20 operates on 64-byte blocks.
const BLOCK_LEN: usize = 64;
/// ChaCha20 key length in bytes.
const KEY_LEN: usize = 32;
/// XChaCha20 extended nonce length in bytes.
const NONCE_LEN: usize = 24;
/// Total seed length in bytes.
const SEED_LEN: usize = KEY_LEN + NONCE_LEN;
/// Global seed stored as `SEED_LEN / 8` independent `u64` words.
const SEED_WORDS: usize = SEED_LEN / 8;

/// Generate keystream block `index` for the given key and nonce.
///
/// Returns an error if `index` runs past the cipher's u32 block counter (2^32
/// blocks, ~256 GiB of stream) -- that's how we signal the DRBG is exhausted.
fn keystream_block(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    index: u64,
) -> Result<[u8; BLOCK_LEN], crate::Error> {
    let mut cipher =
        XChaCha20::new_from_slices(key, nonce).map_err(|_| crate::Error::InvalidSeed)?;
    // chacha20 seeks by byte offset, not by block index.
    let pos = index
        .checked_mul(BLOCK_LEN as u64)
        .ok_or(crate::Error::DrbgExhausted)?;
    cipher
        .try_seek(pos)
        .map_err(|_| crate::Error::DrbgExhausted)?;
    let mut block = [0u8; BLOCK_LEN];
    cipher.apply_keystream(&mut block);
    Ok(block)
}

/// A seedable, deterministic counter-mode ChaCha20 DRBG.
///
/// Local instances are the easy-to-unit-test form of this backend. The
/// process-wide seeded state ([`seed_from_slice`]) is just a thin wrapper around
/// the same keystream construction.
#[derive(Debug, PartialEq, Eq)]
pub struct Drbg {
    key: [u8; KEY_LEN],
    nonce: [u8; NONCE_LEN],
    counter: u64,
}

impl Drbg {
    /// Construct a DRBG from a seed of at least [`SEED_LEN`] bytes.
    ///
    /// First 32 bytes are the key, next 24 are the nonce. Anything past that is
    /// ignored.
    pub fn from_seed(seed: &[u8]) -> Result<Self, crate::Error> {
        if seed.len() < SEED_LEN {
            return Err(crate::Error::InvalidSeed);
        }
        let mut key = [0u8; KEY_LEN];
        let mut nonce = [0u8; NONCE_LEN];
        key.copy_from_slice(&seed[..KEY_LEN]);
        nonce.copy_from_slice(&seed[KEY_LEN..SEED_LEN]);
        Ok(Self {
            key,
            nonce,
            counter: 0,
        })
    }

    /// Fill `dest` with the next bytes of the keystream.
    pub fn fill(&mut self, dest: &mut [u8]) -> Result<(), crate::Error> {
        let blocks = dest.len().div_ceil(BLOCK_LEN);
        let start = self.counter;
        self.counter = self.counter.saturating_add(blocks as u64);
        for i in 0..blocks {
            let block = keystream_block(&self.key, &self.nonce, start + i as u64)?;
            let from = i * BLOCK_LEN;
            let to = core::cmp::min(from + BLOCK_LEN, dest.len());
            dest[from..to].copy_from_slice(&block[..to - from]);
        }
        Ok(())
    }

    /// Fill potentially uninitialized `dest` with keystream bytes.
    pub fn fill_uninit(
        &mut self,
        dest: &mut [core::mem::MaybeUninit<u8>],
    ) -> Result<(), crate::Error> {
        // SAFETY: `MaybeUninit<u8>` has no validity constraints, so writing
        // initialized bytes through an `&mut [u8]` view is always sound.
        let bytes =
            unsafe { core::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut u8, dest.len()) };
        self.fill(bytes)
    }
}

static SEEDED: AtomicBool = AtomicBool::new(false);
static COUNTER: AtomicU64 = AtomicU64::new(0);
// Written out longhand on purpose: array-repeat of a non-Copy type wants inline
// const blocks, and those need rustc >= 1.79 while our workspace floor is 1.75.
// I'm keeping it that low because I don't want Saikuro to be not compatible
// with older toolchains, and 1.75 is the oldest that is reasonable.
static SEED: [AtomicU64; SEED_WORDS] = [
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
];

/// Seed the process-wide DRBG from `seed`.
///
/// Call this once at startup, before any concurrent [`crate::fill`]. Each seed
/// word is stored individually with release ordering, so a reader that sees
/// `SEEDED` will never catch a half-written seed.
pub fn seed_from_slice(seed: &[u8]) -> Result<(), crate::Error> {
    if seed.len() < SEED_LEN {
        return Err(crate::Error::InvalidSeed);
    }
    for (i, word) in SEED.iter().enumerate() {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&seed[i * 8..i * 8 + 8]);
        word.store(u64::from_ne_bytes(bytes), Ordering::Release);
    }
    COUNTER.store(0, Ordering::Relaxed);
    SEEDED.store(true, Ordering::Release);
    Ok(())
}

/// Whether the process-wide DRBG has been seeded.
pub fn is_seeded() -> bool {
    SEEDED.load(Ordering::Acquire)
}

/// Read the process-wide seed as `(key, nonce)`.
fn read_seed() -> ([u8; KEY_LEN], [u8; NONCE_LEN]) {
    let mut seed = [0u8; SEED_LEN];
    for (i, word) in SEED.iter().enumerate() {
        seed[i * 8..i * 8 + 8].copy_from_slice(&word.load(Ordering::Acquire).to_ne_bytes());
    }
    let mut key = [0u8; KEY_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    key.copy_from_slice(&seed[..KEY_LEN]);
    // SEED only starts out zeroed because it's a static. seed_from_slice() writes
    // real entropy into it before anyone calls fill(), and fill() is gated on
    // is_seeded(), so nobody ever actually reads the zero initializer.
    nonce.copy_from_slice(&seed[KEY_LEN..SEED_LEN]);
    (key, nonce)
}

/// Fill `dest` from the process-wide DRBG.
pub fn fill(dest: &mut [u8]) -> Result<(), crate::Error> {
    if !is_seeded() {
        return Err(crate::Error::DrbgNotSeeded);
    }
    let (key, nonce) = read_seed();
    let blocks = dest.len().div_ceil(BLOCK_LEN);
    let start = COUNTER.fetch_add(blocks as u64, Ordering::Relaxed);
    for i in 0..blocks {
        let block = keystream_block(&key, &nonce, start + i as u64)?;
        let from = i * BLOCK_LEN;
        let to = core::cmp::min(from + BLOCK_LEN, dest.len());
        dest[from..to].copy_from_slice(&block[..to - from]);
    }
    Ok(())
}

/// Fill potentially uninitialized `dest` from the process-wide DRBG.
pub fn fill_uninit(dest: &mut [core::mem::MaybeUninit<u8>]) -> Result<(), crate::Error> {
    // SAFETY: `MaybeUninit<u8>` has no validity constraints, so writing
    // initialized bytes through an `&mut [u8]` view is always sound.
    let bytes =
        unsafe { core::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut u8, dest.len()) };
    fill(bytes)
}
