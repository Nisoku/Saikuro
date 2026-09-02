//! Native (host) runner: runs the full portable suite plus host-only tests.

use std::process::ExitCode;

#[path = "../native/mod.rs"]
mod native;

use saikuro_tests::{block_on, register_all, TestFn, TestSuite};

fn main() -> ExitCode {
    let mut suite = TestSuite::new();
    register_all(&mut suite);
    native::register(&mut suite);

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
