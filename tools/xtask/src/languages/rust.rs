// rust

use super::{root, run_fix_step, run_formatter};
use crate::run;

pub(super) fn rust_wasm_clippy_args() -> Vec<String> {
    [
        "clippy",
        "--target",
        "wasm32-unknown-unknown",
        "--no-default-features",
        "--features",
        "wasm",
        "--",
        "-D",
        "warnings",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn rust_format_check() -> anyhow::Result<()> {
    let w = root();
    run_fix_step(
        "Rust workspace",
        &w,
        "cargo",
        &["fmt", "--all", "--", "--check"],
        &["fmt", "--all"],
    )?;
    run_fix_step(
        "Rust adapter",
        &w,
        "cargo",
        &["fmt", "-p", "saikuro", "--", "--check"],
        &["fmt", "-p", "saikuro"],
    )?;
    run_fix_step(
        "Rust xtask",
        &w,
        "cargo",
        &[
            "fmt",
            "--manifest-path",
            "tools/xtask/Cargo.toml",
            "--",
            "--check",
        ],
        &["fmt", "--manifest-path", "tools/xtask/Cargo.toml"],
    )
}

pub(super) fn rust_format() -> anyhow::Result<()> {
    let w = root();
    run_formatter("Rust workspace", &w, "cargo", &["fmt", "--all"])?;
    run_formatter("Rust adapter", &w, "cargo", &["fmt", "-p", "saikuro"])?;
    run_formatter(
        "Rust xtask",
        &w,
        "cargo",
        &["fmt", "--manifest-path", "tools/xtask/Cargo.toml"],
    )
}

pub(super) fn rust_lint() -> anyhow::Result<()> {
    let w = root();
    run::run(
        &w,
        "cargo",
        [
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run::run(
        &w,
        "cargo",
        ["clippy", "-p", "saikuro", "--", "-D", "warnings"],
    )?;
    run::run(
        &w,
        "cargo",
        [
            "clippy",
            "--manifest-path",
            "tools/xtask/Cargo.toml",
            "--",
            "-D",
            "warnings",
        ],
    )
}

pub(super) fn rust_check() -> anyhow::Result<()> {
    let w = root();
    rust_format_check()?;
    rust_lint()?;
    run::cargo(&w, &["test", "--workspace"])?;
    run::cargo(
        &w,
        &rust_wasm_clippy_args()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    )?;
    run::cargo(&w, &["test", "-p", "saikuro"])
}
