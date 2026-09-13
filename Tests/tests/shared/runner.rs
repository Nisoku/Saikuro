//! Shared runner loop used by every target harness except wasm. (cause wasm is special :P)

use alloc::format;

use super::capacity::SKIPPED;
use super::{block_on, TestFn, TestSuite};

/// Execute every test in `suite` and report progress through `write`.
pub fn run(suite: &mut TestSuite, mut write: impl FnMut(&str)) -> u32 {
    write(&format!(
        "registered {} sync, {} async tests",
        suite.count_sync(),
        suite.count_async()
    ));

    let mut skipped: u32 = 0;
    for test in &suite.tests {
        let result = match &test.run {
            TestFn::Sync(f) => f(),
            TestFn::Async(f) => block_on(f()),
        };
        match result {
            Ok(()) => write(&format!("  PASS {}", test.name)),
            Err(e) if e == SKIPPED => {
                write(&format!("  SKIP {}: over capacity", test.name));
                skipped += 1;
            }
            Err(e) => {
                write(&format!("  FAIL {}: {}", test.name, e));
                suite.failed += 1;
                suite.failures.push(test.name);
            }
        }
    }

    let passed = suite.count_sync() + suite.count_async() - suite.failures.len() as u32;
    write(&format!(
        "Results: {passed} passed, {skipped} skipped, {} failed",
        suite.failed
    ));
    if suite.failed > 0 {
        write(&format!("failures: {:?}", suite.failures));
    }
    suite.failed
}
