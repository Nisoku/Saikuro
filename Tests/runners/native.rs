//! Native (host) runner: runs the full portable suite plus host-only tests.

use std::process::ExitCode;

#[path = "../tests/native/mod.rs"]
mod native;

use saikuro_tests::{register_all, run, TestSuite};

fn main() -> ExitCode {
    let mut suite = TestSuite::new();
    register_all(&mut suite);
    native::register(&mut suite);

    let failed = run(&mut suite, |line| println!("{line}"));
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}