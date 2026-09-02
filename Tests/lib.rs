//! Saikuro test suite.

#![no_std]

#[macro_use]
extern crate alloc;

#[path = "tests/shared/mod.rs"]
pub mod shared;

pub use shared::{
    block_on, common, format, register_all, vec, AsyncTestFn, BTreeMap, Box, String, SyncTestFn,
    Test, TestFn, TestSuite, ToOwned, ToString, Vec,
};
