//! Host-only core diagnostics: reports in-memory sizes of the core value
//! types

use saikuro_event::{Value, ValueMap};

pub fn register(suite: &mut saikuro_tests::TestSuite) {
    suite.register("native::core_value_size_report", core_value_size_report);
}

fn core_value_size_report() -> Result<(), &'static str> {
    eprintln!("Value: {} bytes", core::mem::size_of::<Value>());
    eprintln!("ValueMap: {} bytes", core::mem::size_of::<ValueMap>());
    Ok(())
}
