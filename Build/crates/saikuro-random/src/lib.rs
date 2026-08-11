//! Randomness and entropy facade for Saikuro.
//!
//! Wraps the platform entropy source in a small `no_std` API so protocol types
//! don't have to depend on one specific RNG crate.
//!
//! Backend selection works the same way as saikuro-exec: the binary picks its
//! entropy source through cargo features.
//!
//! # Determinism
//! Turn on `drbg` and every call becomes reproducible for a given seed

#![no_std]

#[cfg(feature = "std")]
extern crate std;

use core::mem::MaybeUninit;

// `drbg` is the deterministic override for MCU targets without an OS entropy
// source.  Combining it with a platform backend would compile getrandom for
// nothing and let the drbg implementation win silently, so reject the
// combination at build time and force `--no-default-features --features drbg`.
#[cfg(all(
    feature = "drbg",
    any(feature = "os", feature = "wasm", feature = "custom")
))]
compile_error!(
    "saikuro-random: `drbg` conflicts with the `os`, `wasm`, or `custom` backend; \
     build with `--no-default-features --features drbg`"
);

#[cfg(any(
    all(feature = "os", feature = "wasm"),
    all(feature = "os", feature = "custom"),
    all(feature = "wasm", feature = "custom")
))]
compile_error!("saikuro-random: select exactly one of `os`, `wasm`, or `custom`");

#[cfg(feature = "drbg")]
mod drbg;

pub use uuid::Uuid;

/// Deterministic, seedable ChaCha20 DRBG.
///
/// Comes with the `drbg` feature. Local instances are fully deterministic:
/// the same seed always gives the same stream, which is what makes them handy
/// for reproducible tests and as the entropy core on MCUs with no hardware RNG.
#[cfg(feature = "drbg")]
pub use drbg::Drbg;

/// Errors produced by the entropy facade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The getrandom-based backend couldn't produce entropy.
    #[cfg(any(feature = "os", feature = "wasm", feature = "custom"))]
    Backend(getrandom::Error),
    /// The DRBG was used before anyone seeded it.
    #[cfg(feature = "drbg")]
    DrbgNotSeeded,
    /// The seed handed to the DRBG was too short.
    #[cfg(feature = "drbg")]
    InvalidSeed,
    /// The DRBG keystream for the current seed ran out.
    #[cfg(feature = "drbg")]
    DrbgExhausted,
    /// No entropy backend was selected.
    #[cfg(not(any(feature = "os", feature = "wasm", feature = "custom", feature = "drbg")))]
    NoBackend,
    /// The process-wide DRBG was already initialized.
    #[cfg(feature = "drbg")]
    AlreadySeeded,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            #[cfg(any(feature = "os", feature = "wasm", feature = "custom"))]
            Error::Backend(e) => write!(f, "entropy backend failed: {e}"),
            #[cfg(feature = "drbg")]
            Error::DrbgNotSeeded => write!(f, "DRBG used before being seeded"),
            #[cfg(feature = "drbg")]
            Error::InvalidSeed => write!(f, "DRBG seed must be at least 56 bytes"),
            #[cfg(feature = "drbg")]
            Error::DrbgExhausted => write!(f, "DRBG keystream exhausted; reseed required"),
            #[cfg(feature = "drbg")]
            Error::AlreadySeeded => write!(f, "DRBG has already been seeded"),
            #[cfg(not(any(
                feature = "os",
                feature = "wasm",
                feature = "custom",
                feature = "drbg"
            )))]
            Error::NoBackend => write!(f, "no entropy backend selected"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

#[cfg(any(feature = "os", feature = "wasm", feature = "custom"))]
impl From<getrandom::Error> for Error {
    fn from(err: getrandom::Error) -> Self {
        Error::Backend(err)
    }
}

/// Fill `dest` with cryptographically secure random bytes.
///
/// With the `drbg` feature on you get the DRBG backend instead, and you have to
/// seed it first via [`seed_from_slice`].
pub fn fill(dest: &mut [u8]) -> Result<(), Error> {
    fill_impl(dest)
}

/// Fill potentially uninitialized `dest` with random bytes.
///
/// Same semantics as [`getrandom::fill_uninit`]: on success every byte is
/// initialized, and even on the error path the buffer may be partly written.
pub fn fill_uninit(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    fill_uninit_impl(dest)
}

/// Draw a random `u32` from the active backend.
pub fn u32() -> Result<u32, Error> {
    let mut bytes = [0u8; 4];
    fill(&mut bytes)?;
    Ok(u32::from_ne_bytes(bytes))
}

/// Draw a random `u64` from the active backend.
pub fn u64() -> Result<u64, Error> {
    let mut bytes = [0u8; 8];
    fill(&mut bytes)?;
    Ok(u64::from_ne_bytes(bytes))
}

/// Generate a random RFC 4122 version 4 UUID.
///
/// The 16 random bytes come from the active backend; the version and variant
/// bits get set per RFC 9562 section 5.8.
pub fn uuid_v4() -> Result<Uuid, Error> {
    let mut bytes = [0u8; 16];
    fill(&mut bytes)?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(Uuid::from_bytes(bytes))
}

/// Seed the DRBG backend from `seed`.
///
/// The seed needs at least 56 bytes: the first 32 are the ChaCha20 key and the
/// next 24 are the XChaCha20 nonce. Call it once at startup, before any
/// concurrent `fill` calls, since the seed is written before tasks spawn, readers
/// never catch a half-written seed. Only there with the `drbg` feature.
#[cfg(feature = "drbg")]
pub fn seed_from_slice(seed: &[u8]) -> Result<(), Error> {
    drbg::seed_from_slice(seed)
}

/// Report whether the DRBG backend has been seeded.
#[cfg(feature = "drbg")]
pub fn is_seeded() -> bool {
    drbg::is_seeded()
}

#[cfg(feature = "drbg")]
fn fill_impl(dest: &mut [u8]) -> Result<(), Error> {
    drbg::fill(dest)
}

#[cfg(feature = "drbg")]
fn fill_uninit_impl(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    drbg::fill_uninit(dest)
}

#[cfg(all(
    not(feature = "drbg"),
    any(feature = "os", feature = "wasm", feature = "custom")
))]
fn fill_impl(dest: &mut [u8]) -> Result<(), Error> {
    getrandom::fill(dest).map_err(Error::from)
}

#[cfg(all(
    not(feature = "drbg"),
    any(feature = "os", feature = "wasm", feature = "custom")
))]
fn fill_uninit_impl(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    getrandom::fill_uninit(dest)
        .map_err(Error::from)
        .map(|_| ())
}

#[cfg(not(any(feature = "os", feature = "wasm", feature = "custom", feature = "drbg")))]
fn fill_impl(_dest: &mut [u8]) -> Result<(), Error> {
    Err(Error::NoBackend)
}

#[cfg(not(any(feature = "os", feature = "wasm", feature = "custom", feature = "drbg")))]
fn fill_uninit_impl(_dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    Err(Error::NoBackend)
}
