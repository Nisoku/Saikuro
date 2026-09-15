// python

use anyhow::Context;
use std::io::Write;
use std::path::PathBuf;

use super::{ci, run_fix_step, run_formatter};
use crate::{paths, run};

fn py_dir() -> PathBuf {
    paths::adapters_dir().join("python")
}

pub(super) fn python_format_check() -> anyhow::Result<()> {
    run_fix_step(
        "Python",
        &py_dir(),
        "uv",
        &["run", "ruff", "format", "--check", "."],
        &["run", "ruff", "format", "."],
    )
}

pub(super) fn python_format() -> anyhow::Result<()> {
    run_formatter("Python", &py_dir(), "uv", &["run", "ruff", "format", "."])
}

pub(super) fn python_lint() -> anyhow::Result<()> {
    let dir = py_dir();
    let out = run::run_capture(&dir, "uv", ["run", "ruff", "check", "."])?;
    if out.status.success() {
        return Ok(());
    }
    std::io::stdout().write_all(&out.stdout)?;
    std::io::stderr().write_all(&out.stderr)?;
    if !ci() {
        run::run(&dir, "uv", ["run", "ruff", "check", ".", "--fix"])?;
        println!("[WARN] Python lint issues auto-fixed. Stage changes before committing.");
    }
    anyhow::bail!("python lint failed")
}

pub(super) fn python_test() -> anyhow::Result<()> {
    run::run(&py_dir(), "uv", ["run", "pytest"])
}

pub(super) fn python_setup() -> anyhow::Result<()> {
    run::run(
        &py_dir(),
        "uv",
        ["sync", "--extra", "dev", "--extra", "websocket"],
    )
}

pub(super) fn python_clean() -> anyhow::Result<()> {
    let dir = py_dir();
    for name in [".venv", ".ruff_cache", ".pytest_cache"] {
        let p = dir.join(name);
        if p.is_dir() {
            std::fs::remove_dir_all(&p).with_context(|| format!("rm {}", p.display()))?;
        }
    }
    remove_tree_matching(&dir, |p| {
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or_default();
        name == "__pycache__" || name.ends_with(".egg-info") || name.ends_with(".pyc")
    })
}

fn remove_tree_matching(
    dir: &std::path::Path,
    pred: impl Fn(&std::path::Path) -> bool,
) -> anyhow::Result<()> {
    let mut stack: Vec<PathBuf> = vec![dir.to_path_buf()];
    let mut found: Vec<PathBuf> = Vec::new();
    while let Some(p) = stack.pop() {
        if let Ok(read) = std::fs::read_dir(&p) {
            for entry in read.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path.clone());
                }
                found.push(path);
            }
        }
    }
    for p in found.iter().rev() {
        if pred(p) {
            if p.is_dir() {
                std::fs::remove_dir_all(p).with_context(|| format!("rm {}", p.display()))?;
            } else {
                let _ = std::fs::remove_file(p);
            }
        }
    }
    Ok(())
}
