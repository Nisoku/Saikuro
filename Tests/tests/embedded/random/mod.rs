//! DRBG and UUID tests running on the embedded engines (QEMU)

use saikuro_tests::TestSuite;

/// Deterministic entropy source that fills with a fixed incrementing pattern.
/// No target has an OS RNG in QEMU, so tests seed from this instead of a
/// hardware source.
pub struct FixedEntropy;

impl saikuro_random::EntropySource for FixedEntropy {
    fn try_fill(&self, dest: &mut [u8]) -> Result<(), saikuro_event::SaikuroError> {
        for (i, byte) in dest.iter_mut().enumerate() {
            *byte = i.wrapping_mul(31).wrapping_add(17) as u8;
        }
        Ok(())
    }
}

pub fn register(suite: &mut TestSuite) {
    suite.register("random.try_auto_seed_fails", try_auto_seed_fails);
    suite.register("random.init_from", init_from);
    suite.register("random.uuid_after_seed", uuid_after_seed);
}

fn try_auto_seed_fails() -> Result<(), &'static str> {
    if saikuro_random::is_seeded() {
        return Err("DRBG unexpectedly seeded before init_from");
    }
    let mut buf = [0u8; 16];
    if saikuro_random::fill(&mut buf).is_ok() {
        return Err("fill succeeded before seeding");
    }
    Ok(())
}

fn init_from() -> Result<(), &'static str> {
    saikuro_random::init_from(&FixedEntropy).map_err(|_| "init_from failed")?;
    if !saikuro_random::is_seeded() {
        return Err("DRBG not seeded after init_from");
    }
    Ok(())
}

fn uuid_after_seed() -> Result<(), &'static str> {
    let uuid = saikuro_random::uuid_v4().map_err(|_| "uuid_v4 failed")?;
    let bytes = uuid.as_bytes();
    if bytes[6] >> 4 != 0x4 {
        return Err("UUID not version 4");
    }
    if bytes[8] & 0xc0 != 0x80 {
        return Err("UUID missing RFC 4122 variant bits");
    }
    Ok(())
}
