//! WASI preview1 runner (`wasm32-wasip1`).

#![no_std]
#![no_main]

extern crate alloc;

use alloc::fmt::Write as _;
use alloc::format;
use alloc::string::String;
use core::task::Waker;

#[path = "../tests/wasi/mod.rs"]
mod wasi;

use embassy_time_driver::Driver;
use saikuro_tests::{block_on, register_all, TestFn, TestSuite};

/// Layout-compatible with `__wasi_ciovec_t`.
#[repr(C)]
struct Iovec {
    buf: *const u8,
    buf_len: usize,
}

#[link(wasm_import_module = "wasi_snapshot_preview1")]
extern "C" {
    fn clock_time_get(clock_id: i32, precision: u64, out: *mut u64) -> i32;
    fn fd_write(fd: i32, iovs: *const Iovec, iovs_len: usize, nwritten: *mut usize) -> i32;
    fn proc_exit(code: i32) -> !;
}

/// Clock id for the monotonic clock, per WASI preview1 snapshot 01.
const MONOTONIC_CLOCK_ID: i32 = 1;

const STDOUT_FD: i32 = 1;

/// Embassy time driver for the busy-poll engine.
struct WasiTimeDriver;

impl Driver for WasiTimeDriver {
    fn now(&self) -> u64 {
        let mut ns: u64 = 0;
        // SAFETY: `ns` is a stack out-pointer valid for the duration of the
        // call; `MONOTONIC_CLOCK_ID` is a defined WASI clock.
        unsafe {
            clock_time_get(MONOTONIC_CLOCK_ID, 1, &mut ns);
        }
        ns / 1_000
    }

    fn schedule_wake(&self, _at: u64, waker: &Waker) {
        waker.wake_by_ref();
    }
}

embassy_time_driver::time_driver_impl!(static WASI_TIME_DRIVER: WasiTimeDriver = WasiTimeDriver);

fn write_stdout(bytes: &[u8]) {
    let iovec = Iovec {
        buf: bytes.as_ptr(),
        buf_len: bytes.len(),
    };
    let mut nwritten: usize = 0;
    // SAFETY: `iovec` borrows `bytes` for the duration of the call, and
    // `nwritten` is a stack out-pointer.
    unsafe {
        fd_write(STDOUT_FD, core::ptr::addr_of!(iovec), 1, &mut nwritten);
    }
}

/// Run the full suite, returning the accumulated report and the failure count.
fn run_suite() -> (String, u32) {
    let mut suite = TestSuite::new();
    register_all(&mut suite);
    wasi::register(&mut suite);

    let mut report: String = format!(
        "registered {} sync, {} async tests\n",
        suite.count_sync(),
        suite.count_async()
    );

    for test in &suite.tests {
        let result = match &test.run {
            TestFn::Sync(f) => f(),
            TestFn::Async(f) => block_on(f()),
        };
        match result {
            Ok(()) => {
                let _ = writeln!(report, "  PASS {}", test.name);
            }
            Err(e) => {
                let _ = writeln!(report, "  FAIL {}: {}", test.name, e);
                suite.failed += 1;
                suite.failures.push(test.name);
            }
        }
    }

    let _ = writeln!(
        report,
        "Results: {} passed, {} failed",
        suite.count_sync() + suite.count_async() - suite.failures.len() as u32,
        suite.failed
    );
    if suite.failed > 0 {
        let _ = writeln!(report, "failures: {:?}", suite.failures);
    }
    (report, suite.failed)
}

#[no_mangle]
pub extern "C" fn _start() {
    let (report, failed) = run_suite();
    write_stdout(report.as_bytes());
    let code = if failed == 0 { 0 } else { 1 };
    // SAFETY: terminating the WASI process is the intended final action.
    unsafe { proc_exit(code) }
}

// `crt1-command.o` is disabled for this no_std command (see
// `.cargo/config.toml`), so no libc provides the memcmp required by
// alloc/serde code. Implement it against the same ABI as wasi-libc.
#[no_mangle]
pub unsafe extern "C" fn memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    // SAFETY: callers pass valid, in-bounds buffers of length `n`.
    unsafe {
        let sa = core::slice::from_raw_parts(a, n);
        let sb = core::slice::from_raw_parts(b, n);
        for (x, y) in sa.iter().zip(sb.iter()) {
            if x != y {
                return (*x as i32) - (*y as i32);
            }
        }
    }
    0
}