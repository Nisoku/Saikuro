//! Host-side runner for the embedded-engine.

#[macro_use]
extern crate alloc;

use std::io::{self, Write as _};
use std::sync::LazyLock;
use std::time::Instant;

use critical_section::set_impl;
use embassy_time_driver::{time_driver_impl, Driver};
use saikuro_event::log::LogLevel;
use saikuro_event::{LogRecord, SaikuroError};
use saikuro_random::EntropySource;

use saikuro_tests::{register_all, run, TestSuite};

/// No-op critical section.
mod host_critical_section {
    use super::*;

    struct NoopCriticalSection;

    set_impl!(NoopCriticalSection);

    // SAFETY: single-threaded host context, no interrupts or preemption points.
    unsafe impl critical_section::Impl for NoopCriticalSection {
        unsafe fn acquire() -> critical_section::RawRestoreState {
            Default::default()
        }

        unsafe fn release(_restore_state: critical_section::RawRestoreState) {}
    }
}

/// Busy-poll embassy-time driver for the host.
struct HostTimeDriver;

impl Driver for HostTimeDriver {
    fn now(&self) -> u64 {
        HOST_EPOCH.elapsed().as_micros() as u64
    }

    fn schedule_wake(&self, _at: u64, waker: &core::task::Waker) {
        waker.wake_by_ref();
    }
}

time_driver_impl!(static HOST_TIME_DRIVER: HostTimeDriver = HostTimeDriver);

static HOST_EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);

struct HostEntropy;

impl EntropySource for HostEntropy {
    fn try_fill(&self, dest: &mut [u8]) -> Result<(), SaikuroError> {
        for (i, byte) in dest.iter_mut().enumerate() {
            *byte = i.wrapping_mul(31).wrapping_add(17) as u8;
        }
        Ok(())
    }
}

#[path = "../tests/embedded/mod.rs"]
pub mod embedded;

/// Emit `msg` as an info-level log record.
fn log_line(msg: &str) {
    let record = LogRecord::now(LogLevel::Info, "embedded-host", msg);
    let mut stdout = io::stdout();
    let _ = writeln!(stdout, "{record}");
}

fn main() {
    if let Err(e) = saikuro_random::init_from(&HostEntropy) {
        log_line(&format!("FATAL: DRBG seeding failed: {e}"));
        std::process::exit(1);
    }

    let mut suite = TestSuite::new();
    register_all(&mut suite);
    embedded::register(&mut suite);

    log_line("embedded-host suite on the `base` executor (Miri-validated)");
    let failed = run(&mut suite, |line| log_line(line));
    if failed != 0 {
        std::process::exit(1);
    }
}
