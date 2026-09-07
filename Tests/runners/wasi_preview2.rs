//! WASI preview2 runner (`wasm32-wasip2`)

use std::process::ExitCode;
use std::sync::OnceLock;
use std::time::Instant;

use core::task::Waker;

#[path = "../tests/wasi/mod.rs"]
mod wasi;

use embassy_time_driver::Driver;
use saikuro_tests::{register_all, run, TestSuite};

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

    let failed = run(&mut suite, |line| println!("{line}"));
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}