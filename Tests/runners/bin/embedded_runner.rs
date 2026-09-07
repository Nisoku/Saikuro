//! Embedded QEMU test runner shared by all five `bin` binaries.

use alloc::format;
use core::fmt::Write as _;
use core::task::Waker;

use embassy_time_driver::{time_driver_impl, Driver};
use saikuro_event::log::LogLevel;
use saikuro_event::LogRecord;
use saikuro_event::SaikuroError;
use saikuro_random::EntropySource;

use saikuro_tests::{register_all, run, TestSuite};

#[path = "../../tests/embedded/mod.rs"]
pub mod embedded;

#[path = "console.rs"]
mod console;
pub use console::Console;
#[cfg(target_arch = "riscv32")]
pub use console::qemu_exit;

/// Busy-poll embassy-time driver for QEMU.
struct QemuTimeDriver;

impl Driver for QemuTimeDriver {
    fn now(&self) -> u64 {
        console::time_now_us()
    }

    fn schedule_wake(&self, _at: u64, waker: &Waker) {
        waker.wake_by_ref();
    }
}

time_driver_impl!(static QEMU_TIME_DRIVER: QemuTimeDriver = QemuTimeDriver);

/// Deterministic entropy source for the QEMU fleet.
///
/// No guest machine (mps2-an385, mps2-an505, virt) exposes a TRNG omg
struct QemuEntropy;

impl EntropySource for QemuEntropy {
    fn try_fill(&self, dest: &mut [u8]) -> Result<(), SaikuroError> {
        for (i, byte) in dest.iter_mut().enumerate() {
            *byte = i.wrapping_mul(31).wrapping_add(17) as u8;
        }
        Ok(())
    }
}

/// Emit `msg` as an info-level log record on the semihosting console.
fn log_line(msg: &str) {
    let record = LogRecord::now(LogLevel::Info, "qemu", msg);
    let mut console = Console::new();
    let _ = writeln!(console, "{record}");
}

/// Run the full embedded suite and return the number of failures.
pub fn run_qemu_tests(banner: &str) -> u32 {
    if let Err(e) = saikuro_random::init_from(&QemuEntropy) {
        log_line(&format!("FATAL: DRBG seeding failed: {e}"));
        return 1;
    }

    let mut suite = TestSuite::new();
    register_all(&mut suite);
    embedded::register(&mut suite);

    log_line(banner);
    run(&mut suite, |line| log_line(line))
}