//! WASI preview2 runner (`wasm32-wasip2`)

use std::process::ExitCode;
use std::sync::OnceLock;
use std::time::Instant;

use core::task::Waker;

#[path = "../tests/wasi/mod.rs"]
mod wasi;

use embassy_time_driver::Driver;
use saikuro_tests::{block_on, register_all, TestFn, TestSuite};

/// Embassy time driver for the busy-poll engine.
struct WasiTimeDriver;

impl Driver for WasiTimeDriver {
    fn now(&self) -> u64 {
        static START: OnceLock<Instant> = OnceLock::new();
        START.get_or_init(Instant::now).elapsed().as_micros() as u64
    }

    fn schedule_wake(&self, _at: u64, waker: &Waker) {
        waker.wake_by_ref();
    }
}

embassy_time_driver::time_driver_impl!(static WASI_TIME_DRIVER: WasiTimeDriver = WasiTimeDriver);

fn main() -> ExitCode {
    let mut suite = TestSuite::new();
    register_all(&mut suite);
    wasi::register(&mut suite);

    let total_sync = suite.count_sync();
    let total_async = suite.count_async();
    println!("registered {total_sync} sync, {total_async} async tests");

    for test in &suite.tests {
        let result = match &test.run {
            TestFn::Sync(f) => f(),
            TestFn::Async(f) => block_on(f()),
        };
        match result {
            Ok(()) => println!("  PASS {}", test.name),
            Err(e) => {
                println!("  FAIL {}: {}", test.name, e);
                suite.failed += 1;
                suite.failures.push(test.name);
            }
        }
    }

    let passed = total_sync + total_async - suite.failures.len() as u32;
    println!("Results: {passed} passed, {} failed", suite.failed);

    if suite.failed == 0 {
        ExitCode::SUCCESS
    } else {
        println!("failures: {:?}", suite.failures);
        ExitCode::FAILURE
    }
}