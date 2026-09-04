//! Embedded QEMU test runner shared by all five `bin` binaries.

use alloc::format;
use core::fmt::Write as _;

use saikuro_event::log::LogLevel;
use saikuro_event::LogRecord;

use saikuro_tests::{block_on, register_all, TestFn, TestSuite};

#[path = "../../tests/embedded/mod.rs"]
pub mod embedded;

#[path = "console.rs"]
mod console;
pub use console::Console;
#[cfg(target_arch = "riscv32")]
pub use console::qemu_exit;

/// Emit `msg` as an info-level log record on the semihosting console.
fn log_line(msg: &str) {
    let record = LogRecord::now(LogLevel::Info, "qemu", msg);
    let mut console = Console::new();
    let _ = writeln!(console, "{record}");
}

/// Run the full embedded suite and return the number of failures.
pub fn run_qemu_tests(banner: &str) -> u32 {
    let mut suite = TestSuite::new();
    register_all(&mut suite);
    embedded::register(&mut suite);

    log_line(banner);
    log_line(&format!(
        "registered {} sync, {} async tests",
        suite.count_sync(),
        suite.count_async()
    ));

    for test in &suite.tests {
        let result = match &test.run {
            TestFn::Sync(f) => f(),
            TestFn::Async(f) => block_on(f()),
        };
        match result {
            Ok(()) => log_line(&format!("  PASS {}", test.name)),
            Err(e) => {
                log_line(&format!("  FAIL {}: {}", test.name, e));
                suite.failed += 1;
                suite.failures.push(test.name);
            }
        }
    }

    let passed = suite.count_sync() + suite.count_async() - suite.failures.len() as u32;
    log_line(&format!("Results: {passed} passed, {} failed", suite.failed));
    if suite.failed > 0 {
        log_line(&format!("failures: {:?}", suite.failures));
    }
    suite.failed
}
