//! Portable, engine-agnostic test suite.

pub use alloc::borrow::ToOwned;
pub use alloc::boxed::Box;
pub use alloc::collections::BTreeMap;
pub use alloc::format;
pub use alloc::string::{String, ToString};
pub use alloc::vec;
pub use alloc::vec::Vec;

use ::core::future::Future;
use ::core::pin::Pin;

pub mod capacity;
pub mod codegen;
pub mod common;
pub mod core;
pub mod exec;
pub mod router;
pub mod runner;
pub mod runtime;
pub mod schema;
pub mod storage;
pub mod transport;
pub mod wire;

/// Safe assertion macro for no_std tests that returns `Err(&str)` instead of
/// panicking, so failures surface through the [`TestSuite`] runner.
#[macro_export]
macro_rules! check_test {
    ($left:expr, $msg:expr $(,)?) => {
        if !$left {
            return Err($msg);
        }
    };
}

/// Run a future to completion using the active engine.
pub fn block_on<F>(fut: F) -> F::Output
where
    F: Future + 'static,
    F::Output: 'static,
{
    saikuro_exec::block_on(Box::pin(fut))
}

/// Register a sync shared test and, on wasm, expose it as its own
/// `wasm-bindgen-test` entry.
#[macro_export]
macro_rules! shared_test {
    ($suite:expr, $name:literal, $run:ident $(,)?) => {
        $suite.register($name, $run);

        #[cfg(all(target_arch = "wasm32", test))]
        mod $run {
            #[wasm_bindgen_test::wasm_bindgen_test]
            fn $run() {
                match super::$run() {
                    Ok(()) => {}
                    Err(e) => panic!("{} failed: {}", $name, e),
                }
            }
        }
    };
}

/// Async counterpart to [`shared_test`]
#[macro_export]
macro_rules! shared_test_async {
    ($suite:expr, $name:literal, $run:ident $(,)?) => {
        $suite.register_async($name, $run);

        #[cfg(all(target_arch = "wasm32", test))]
        mod $run {
            #[wasm_bindgen_test::wasm_bindgen_test]
            async fn $run() {
                match $crate::block_on(super::$run()) {
                    Ok(()) => {}
                    Err(e) => panic!("{} failed: {}", $name, e),
                }
            }
        }
    };
}

pub type SyncTestFn = fn() -> Result<(), &'static str>;
pub type AsyncTestFn = fn() -> Pin<Box<dyn Future<Output = Result<(), &'static str>> + 'static>>;

pub enum TestFn {
    Sync(SyncTestFn),
    Async(AsyncTestFn),
}

pub struct Test {
    pub name: &'static str,
    pub run: TestFn,
}

pub struct TestSuite {
    pub tests: Vec<Test>,
    pub passed: u32,
    pub failed: u32,
    pub failures: Vec<&'static str>,
}

impl Default for TestSuite {
    fn default() -> Self {
        Self::new()
    }
}

impl TestSuite {
    pub fn new() -> Self {
        Self {
            tests: Vec::new(),
            passed: 0,
            failed: 0,
            failures: Vec::new(),
        }
    }

    pub fn register(&mut self, name: &'static str, run: SyncTestFn) {
        self.tests.push(Test {
            name,
            run: TestFn::Sync(run),
        });
    }

    pub fn register_async(&mut self, name: &'static str, run: AsyncTestFn) {
        self.tests.push(Test {
            name,
            run: TestFn::Async(run),
        });
    }

    pub fn run_sync(&mut self) {
        for test in &self.tests {
            match &test.run {
                TestFn::Sync(f) => match f() {
                    Ok(()) => self.passed += 1,
                    Err(_) => {
                        self.failed += 1;
                        self.failures.push(test.name);
                    }
                },
                TestFn::Async(_) => {}
            }
        }
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.tests.iter().map(|t| t.name).collect()
    }

    pub fn count_sync(&self) -> u32 {
        self.tests
            .iter()
            .filter(|t| matches!(t.run, TestFn::Sync(_)))
            .count() as u32
    }

    pub fn count_async(&self) -> u32 {
        self.tests
            .iter()
            .filter(|t| matches!(t.run, TestFn::Async(_)))
            .count() as u32
    }
}

pub fn register_all(suite: &mut TestSuite) {
    core::register(suite);
    exec::register(suite);
    router::register(suite);
    runtime::register(suite);
    schema::register(suite);
    storage::register(suite);
    transport::register(suite);
    codegen::register(suite);
    wire::register(suite);
}
