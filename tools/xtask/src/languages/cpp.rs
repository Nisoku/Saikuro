// cpp

use anyhow::Context;
use std::path::PathBuf;

use super::{ci, collect_recursive};
use crate::{paths, run};

fn cpp_dir() -> PathBuf {
    paths::adapters_dir().join("cpp")
}

fn cpp_sources() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for sub in ["src", "include", "tests"] {
        out.extend(collect_recursive(
            &cpp_dir().join(sub),
            &["*.cpp", "*.hpp", "*.h"],
        ));
    }
    out
}

fn cpp_format_check() -> anyhow::Result<()> {
    if !run::which("clang-format") {
        println!("[WARN] clang-format not found; skipping C++ format check");
        return Ok(());
    }
    let sources = cpp_sources();
    if sources.is_empty() {
        return Ok(());
    }
    let mut check = vec!["--dry-run".to_string(), "-Werror".to_string()];
    check.extend(sources.iter().map(|s| s.display().to_string()));
    let mut fix = vec!["-i".to_string()];
    fix.extend(sources.iter().map(|s| s.display().to_string()));
    run::run(
        &cpp_dir(),
        "clang-format",
        check.iter().map(String::as_str).collect::<Vec<_>>(),
    )
    .with_context(|| "C++ format check".to_string())
    .or_else(|_| {
        if ci() {
            Err(anyhow::anyhow!("C++ format check failed"))
        } else {
            run::run(
                &cpp_dir(),
                "clang-format",
                fix.iter().map(String::as_str).collect::<Vec<_>>(),
            )?;
            println!("[WARN] C++ format issues auto-fixed. Stage changes before committing.");
            Err(anyhow::anyhow!("C++ format check failed"))
        }
    })
}

pub(super) fn cpp_format() -> anyhow::Result<()> {
    if !run::which("clang-format") {
        println!("[WARN] clang-format not found; skipping C++ format");
        return Ok(());
    }
    let sources = cpp_sources();
    if sources.is_empty() {
        return Ok(());
    }
    let mut args = vec!["-i".to_string()];
    args.extend(sources.iter().map(|s| s.display().to_string()));
    run::run(
        &cpp_dir(),
        "clang-format",
        args.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

pub(super) fn cpp_setup() -> anyhow::Result<()> {
    run::run(&cpp_dir(), "cmake", ["-S", ".", "-B", "build"])
}

pub(super) fn cpp_test() -> anyhow::Result<()> {
    run::run(&cpp_dir(), "cmake", ["--build", "build"])?;
    run::run(
        &cpp_dir(),
        "ctest",
        ["--test-dir", "build", "--output-on-failure"],
    )
}

pub(super) fn cpp_check() -> anyhow::Result<()> {
    cpp_format_check()?;
    cpp_setup()?;
    cpp_test()
}

pub(super) fn cpp_clean() -> anyhow::Result<()> {
    let p = cpp_dir().join("build");
    if p.is_dir() {
        std::fs::remove_dir_all(&p)?;
    }
    Ok(())
}
