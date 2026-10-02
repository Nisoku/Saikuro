//! Saikuro test suite.

#![no_std]

#[macro_use]
extern crate alloc;

#[path = "tests/shared/mod.rs"]
pub mod shared;

pub use shared::{
    block_on, capacity, common, format, register_all, vec, AsyncTestFn, BTreeMap, Box, String,
    SyncTestFn, Test, TestFn, TestSuite, ToOwned, ToString, Vec,
};

pub use shared::runner::run;

/// Blocking entry point for the Asyncify e2e gate.
#[cfg(all(target_arch = "wasm32", feature = "asyncify"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn run_asyncify_suite() -> String {
    console_error_panic_hook::set_once();

    let mut suite = TestSuite::new();
    shared::exec::register(&mut suite);

    let mut report = String::new();
    let failed = run(&mut suite, |line| report.push_str(&format!("{line}\n")));
    report.push_str(&format!("asyncify_failed={failed}"));
    report
}
