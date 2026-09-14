// c

use std::path::PathBuf;

use super::{collect_recursive, root, run_fix_step, run_formatter};
use crate::{paths, run};

fn c_dir() -> PathBuf {
    paths::adapters_dir().join("c")
}

fn c_sources() -> Vec<PathBuf> {
    collect_recursive(&c_dir().join("include"), &["*.h"])
}

fn c_format_check() -> anyhow::Result<()> {
    if run::which("clang-format") {
        let sources = c_sources();
        if !sources.is_empty() {
            let mut check = vec!["--dry-run".to_string(), "-Werror".to_string()];
            check.extend(sources.iter().map(|s| s.display().to_string()));
            let mut fix = vec!["-i".to_string()];
            fix.extend(sources.iter().map(|s| s.display().to_string()));
            run_fix_step(
                "C headers",
                &c_dir(),
                "clang-format",
                &check.iter().map(String::as_str).collect::<Vec<_>>(),
                &fix.iter().map(String::as_str).collect::<Vec<_>>(),
            )?;
        }
    }
    run_fix_step(
        "C crate Rust",
        &root(),
        "cargo",
        &["fmt", "-p", "saikuro-c", "--", "--check"],
        &["fmt", "-p", "saikuro-c"],
    )
}

pub(super) fn c_format() -> anyhow::Result<()> {
    if run::which("clang-format") {
        let sources = c_sources();
        if !sources.is_empty() {
            let mut args = vec!["-i".to_string()];
            args.extend(sources.iter().map(|s| s.display().to_string()));
            run::run(
                &c_dir(),
                "clang-format",
                args.iter().map(String::as_str).collect::<Vec<_>>(),
            )?;
        }
    }
    run_formatter(
        "C crate Rust",
        &root(),
        "cargo",
        &["fmt", "-p", "saikuro-c"],
    )
}

pub(super) fn c_check() -> anyhow::Result<()> {
    c_format_check()?;
    run::cargo(&root(), &["build", "-p", "saikuro-c"])?;
    run::cargo(&root(), &["test", "-p", "saikuro-c"])
}
