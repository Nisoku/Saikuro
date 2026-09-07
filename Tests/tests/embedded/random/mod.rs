//! DRBG behaviour on the embedded engines (QEMU).

use saikuro_tests::TestSuite;

pub fn register(suite: &mut TestSuite) {
    suite.register("random.seeded_before_suite", seeded_before_suite);
    suite.register("random.uuid_after_seed", uuid_after_seed);
}

fn seeded_before_suite() -> Result<(), &'static str> {
    if !saikuro_random::is_seeded() {
        return Err("DRBG not seeded by the QEMU runner before the suite");
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