//! The process-wide DRBG must refuse to draw before seeding.
//!
//! This lives in its own test binary so its statics start unseeded btw

#![cfg(feature = "drbg")]

use saikuro_random::{fill, is_seeded, Error};

#[test]
fn unseeded_fill_errors() {
    assert!(!is_seeded());
    let mut buf = [0u8; 16];
    assert_eq!(fill(&mut buf), Err(Error::DrbgNotSeeded));
}
