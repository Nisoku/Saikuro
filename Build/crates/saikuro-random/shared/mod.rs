use core::mem::MaybeUninit;

use chacha20::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
use chacha20::XChaCha20;
use portable_atomic::{AtomicBool, AtomicU64, Ordering};
use rand_core::{CryptoRng, RngCore, SeedableRng};
use saikuro_event::SaikuroError;

pub use uuid::Uuid;

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
/// ChaCha20 exposes a 32-bit block counter for each key and nonce.
const MAX_BLOCKS: u64 = 1u64 << 32;

/// Entropy source for the process-wide DRBG.
pub trait EntropySource {
    /// Fill `dest` with fresh entropy, fully initializing every byte.
    fn try_fill(&self, dest: &mut [u8]) -> Result<(), SaikuroError>;
}

/// Generate keystream block `index` for the given key and nonce.
fn keystream_block(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    index: u64,
) -> Result<[u8; BLOCK_LEN], SaikuroError> {
    let mut cipher = XChaCha20::new_from_slices(key, nonce).map_err(|_| {
        SaikuroError::Entropy(format!("DRBG seed must be at least {SEED_LEN} bytes"))
    })?;
    // chacha20 seeks by byte offset, not by block index.
    let pos = index
        .checked_mul(BLOCK_LEN as u64)
        .ok_or(SaikuroError::Entropy("DRBG keystream exhausted".into()))?;
    cipher
        .try_seek(pos)
        .map_err(|_| SaikuroError::Entropy("DRBG keystream exhausted".into()))?;
    let mut block = [0u8; BLOCK_LEN];
    cipher.apply_keystream(&mut block);
    Ok(block)
}

/// A seedable, deterministic counter-mode ChaCha20 DRBG.
#[derive(Debug, PartialEq, Eq)]
pub struct Drbg {
    key: [u8; KEY_LEN],
    nonce: [u8; NONCE_LEN],
    counter: u64,
}

impl Drbg {
    /// Construct a DRBG from a seed of at least [`SEED_LEN`] bytes.
    ///
    /// The first 32 bytes are the key and the next 24 are the XChaCha20 nonce;
    /// anything past that is ignored.
    pub fn from_seed(seed: &[u8]) -> Result<Self, SaikuroError> {
        if seed.len() < SEED_LEN {
            return Err(SaikuroError::Entropy(format!(
                "DRBG seed must be at least {SEED_LEN} bytes"
            )));
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
    pub fn fill(&mut self, dest: &mut [u8]) -> Result<(), SaikuroError> {
        let blocks = dest.len().div_ceil(BLOCK_LEN);
        let start = self.counter;
        let block_count = blocks as u64;
        let end = start
            .checked_add(block_count)
            .filter(|&end| end <= MAX_BLOCKS)
            .ok_or(SaikuroError::Entropy("DRBG keystream exhausted".into()))?;
        self.counter = end;
        for i in 0..blocks {
            let block = keystream_block(&self.key, &self.nonce, start + (i as u64))?;
            let from = i * BLOCK_LEN;
            let to = core::cmp::min(from + BLOCK_LEN, dest.len());
            dest[from..to].copy_from_slice(&block[..to - from]);
        }
        Ok(())
    }

    /// Fill potentially uninitialized `dest` with keystream bytes.
    pub fn fill_uninit(&mut self, dest: &mut [MaybeUninit<u8>]) -> Result<(), SaikuroError> {
        // SAFETY: `MaybeUninit<u8>` has no validity constraints, so writing
        // initialized bytes through an `&mut [u8]` view is always sound.
        let bytes =
            unsafe { core::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut u8, dest.len()) };
        self.fill(bytes)
    }
}

impl RngCore for Drbg {
    fn next_u32(&mut self) -> u32 {
        let mut bytes = [0u8; 4];
        self.fill(&mut bytes).expect("Drbg keystream exhausted");
        u32::from_ne_bytes(bytes)
    }

    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0u8; 8];
        self.fill(&mut bytes).expect("Drbg keystream exhausted");
        u64::from_ne_bytes(bytes)
    }

    fn fill_bytes(&mut self, dst: &mut [u8]) {
        self.fill(dst).expect("Drbg keystream exhausted");
    }
}

impl CryptoRng for Drbg {}

/// Seed wrapper for `rand_core::SeedableRng`.
#[derive(Clone)]
pub struct SeedBytes(pub [u8; SEED_LEN]);

impl Default for SeedBytes {
    fn default() -> Self {
        SeedBytes([0u8; SEED_LEN])
    }
}

impl AsRef<[u8]> for SeedBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for SeedBytes {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl SeedableRng for Drbg {
    type Seed = SeedBytes;

    fn from_seed(seed: SeedBytes) -> Self {
        Drbg::from_seed(&seed.0).expect("SeedBytes is always SEED_LEN long")
    }
}

static SEEDED: AtomicBool = AtomicBool::new(false);
static INITIALIZING: AtomicBool = AtomicBool::new(false);
static COUNTER: AtomicU64 = AtomicU64::new(0);
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
pub fn seed_from_slice(seed: &[u8]) -> Result<(), SaikuroError> {
    if seed.len() < SEED_LEN {
        return Err(SaikuroError::Entropy(format!(
            "DRBG seed must be at least {SEED_LEN} bytes"
        )));
    }
    if SEEDED.load(Ordering::Acquire) {
        return Ok(());
    }
    if INITIALIZING
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        for _ in 0..1_000_000u32 {
            if SEEDED.load(Ordering::Acquire) {
                return Ok(());
            }
            core::hint::spin_loop();
        }
        return Err(SaikuroError::Entropy("DRBG has already been seeded".into()));
    }
    for (i, word) in SEED.iter().enumerate() {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&seed[i * 8..i * 8 + 8]);
        word.store(u64::from_ne_bytes(bytes), Ordering::Release);
    }
    COUNTER.store(0, Ordering::Relaxed);
    SEEDED.store(true, Ordering::Release);
    INITIALIZING.store(false, Ordering::Release);
    Ok(())
}

/// Seed the process-wide DRBG from an [`EntropySource`].
///
/// Convenience over [`seed_from_slice`]: draw a fresh seed from `source` and
/// install it. Engines expose `init_default`/`init_from` which call this.
pub fn init(source: &impl EntropySource) -> Result<(), SaikuroError> {
    let mut seed = [0u8; SEED_LEN];
    source.try_fill(&mut seed)?;
    seed_from_slice(&seed)
}

/// Whether the process-wide DRBG has been seeded.
pub fn is_seeded() -> bool {
    SEEDED.load(Ordering::Acquire)
}

fn read_seed() -> ([u8; KEY_LEN], [u8; NONCE_LEN]) {
    let mut seed = [0u8; SEED_LEN];
    for (i, word) in SEED.iter().enumerate() {
        seed[i * 8..i * 8 + 8].copy_from_slice(&word.load(Ordering::Acquire).to_ne_bytes());
    }
    let mut key = [0u8; KEY_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    key.copy_from_slice(&seed[..KEY_LEN]);
    // SEED is only zeroed because it's a static; seed_from_slice writes real
    // entropy before any fill, and fill is gated on is_seeded(), so the zero
    // initializer is never read.
    nonce.copy_from_slice(&seed[KEY_LEN..SEED_LEN]);
    (key, nonce)
}

/// Fill `dest` with cryptographically secure random bytes from the
/// process-wide DRBG.
#[expect(unused)]
pub fn fill(dest: &mut [u8]) -> Result<(), SaikuroError> {
    if !is_seeded() {
        crate::try_auto_seed()?;
        if !is_seeded() {
            return Err(SaikuroError::Entropy(
                "DRBG used before being seeded".into(),
            ));
        }
    }
    let (key, nonce) = read_seed();
    let blocks = dest.len().div_ceil(BLOCK_LEN);
    let start = reserve_blocks(blocks as u64)?;
    for i in 0..blocks {
        let block = keystream_block(&key, &nonce, start + (i as u64))?;
        let from = i * BLOCK_LEN;
        let to = core::cmp::min(from + BLOCK_LEN, dest.len());
        dest[from..to].copy_from_slice(&block[..to - from]);
    }
    Ok(())
}

/// Fill potentially uninitialized `dest` with random bytes from the
/// process-wide DRBG.
pub fn fill_uninit(dest: &mut [MaybeUninit<u8>]) -> Result<(), SaikuroError> {
    // SAFETY: `MaybeUninit<u8>` has no validity constraints, so writing
    // initialized bytes through an `&mut [u8]` view is always sound.
    let bytes =
        unsafe { core::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut u8, dest.len()) };
    fill(bytes)
}

/// Draw a random `u32` from the process-wide DRBG.
pub fn u32() -> Result<u32, SaikuroError> {
    let mut bytes = [0u8; 4];
    fill(&mut bytes)?;
    Ok(u32::from_ne_bytes(bytes))
}

/// Draw a random `u64` from the process-wide DRBG.
pub fn u64() -> Result<u64, SaikuroError> {
    let mut bytes = [0u8; 8];
    fill(&mut bytes)?;
    Ok(u64::from_ne_bytes(bytes))
}

/// Generate a random RFC 4122 version 4 UUID from the process-wide DRBG.
pub fn uuid_v4() -> Result<Uuid, SaikuroError> {
    let mut bytes = [0u8; 16];
    fill(&mut bytes)?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(Uuid::from_bytes(bytes))
}

fn reserve_blocks(blocks: u64) -> Result<u64, SaikuroError> {
    let mut current = COUNTER.load(Ordering::Relaxed);
    loop {
        let next = current
            .checked_add(blocks)
            .filter(|&next| next <= MAX_BLOCKS)
            .ok_or(SaikuroError::Entropy("DRBG keystream exhausted".into()))?;
        match COUNTER.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => {
                return Ok(current);
            }
            Err(observed) => {
                current = observed;
            }
        }
    }
}
