//! Randomness and entropy facade for Saikuro.
//!
//! Hides the platform entropy source behind a small `no_std` API so that
//! protocol types do not depend on a specific RNG crate.
//!
//! Backend selection mirrors saikuro-exec: a binary selects its
//! entropy source with cargo features
//!
//! # Determinism
//!
//! Enabling `drbg` makes every call reproducible for a given seed, which is
//! the only way to write deterministic tests over code that issues UUIDs.

#![no_std]

#[cfg(feature = "std")]
extern crate std;

use core::mem::MaybeUninit;

#[cfg(feature = "drbg")]
mod drbg;

pub use uuid::Uuid;

/// Deterministic, seedable ChaCha20 DRBG.
///
/// Available with the `drbg` feature. Local instances are fully deterministic:
/// identical seeds produce identical streams, which makes them usable in
/// reproducible tests and as the entropy core for MCUs without a hardware RNG.
#[cfg(feature = "drbg")]
pub use drbg::Drbg;

/// Errors produced by the entropy facade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The getrandom-based backend failed to produce entropy.
    #[cfg(any(feature = "os", feature = "wasm", feature = "custom"))]
    Backend(getrandom::Error),
    /// The DRBG backend was used before being seeded.
    #[cfg(feature = "drbg")]
    DrbgNotSeeded,
    /// The seed passed to the DRBG was too short.
    #[cfg(feature = "drbg")]
    InvalidSeed,
    /// The DRBG keystream for the current seed has been exhausted.
    #[cfg(feature = "drbg")]
    DrbgExhausted,
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
/// With the `drbg` feature the DRBG backend is used instead; it must be
/// seeded first via [`seed_from_slice`].
pub fn fill(dest: &mut [u8]) -> Result<(), Error> {
    fill_impl(dest)
}

/// Fill potentially uninitialized `dest` with random bytes.
///
/// Semantics match [`getrandom::fill_uninit`]: every byte is initialized on
/// success, even in error paths the buffer may be partially written.
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
/// The 16 random bytes come from the active backend; version and variant bits
/// are set per RFC 9562 section 5.8.
pub fn uuid_v4() -> Result<Uuid, Error> {
    let mut bytes = [0u8; 16];
    fill(&mut bytes)?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(Uuid::from_bytes(bytes))
}

/// Seed the DRBG backend from `seed`.
///
/// The seed must be at least 56 bytes; the first 32 bytes form the ChaCha20
/// key and the next 24 the XChaCha20 nonce. Call once at startup, before any
/// concurrent `fill` call (the seed bytes are written before tasks spawn, so
/// readers never observe a partially-written seed). Only available with the
/// `drbg` feature.
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

#[cfg(all(
    not(feature = "os"),
    not(feature = "wasm"),
    not(feature = "custom"),
    not(feature = "drbg")
))]
compile_error!(
    "saikuro-random requires exactly one entropy backend: enable `os`, `wasm`, `custom`, or `drbg`"
);
