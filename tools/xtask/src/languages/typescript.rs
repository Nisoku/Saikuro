// typescript

use anyhow::Context;
use std::io::Write;
use std::path::PathBuf;

use super::{ci, run_fix_step, run_formatter};
use crate::{paths, run};

pub(super) fn ts_dir() -> PathBuf {
    paths::adapters_dir().join("typescript")
}

fn typescript_format_check() -> anyhow::Result<()> {
    run_fix_step(
        "TypeScript",
        &ts_dir(),
        "npm",
        &["run", "format:check"],
        &["run", "format"],
    )
}

pub(super) fn typescript_format() -> anyhow::Result<()> {
    run_formatter("TypeScript", &ts_dir(), "npm", &["run", "format"])
}

pub(super) fn typescript_lint() -> anyhow::Result<()> {
    let dir = ts_dir();
    let out = run::run_capture(&dir, "npm", ["run", "lint"])?;
    if out.status.success() {
        return Ok(());
    }
    std::io::stdout().write_all(&out.stdout)?;
    std::io::stderr().write_all(&out.stderr)?;
    if !ci() {
        run::run(&dir, "npm", ["run", "lint:fix"])?;
        println!("[WARN] TypeScript lint issues auto-fixed. Stage changes before committing.");
    }
    anyhow::bail!("typescript lint failed")
}

pub(super) fn typescript_check() -> anyhow::Result<()> {
    typescript_format_check()?;
    typescript_lint()?;
    run::run(&ts_dir(), "npm", ["run", "typecheck"])?;
    run::run(&ts_dir(), "npm", ["test"])?;
    run::run(&ts_dir(), "npm", ["run", "build"])
}

pub(super) fn typescript_clean() -> anyhow::Result<()> {
    let dir = ts_dir();
    for name in ["node_modules", "dist"] {
        let p = dir.join(name);
        if p.is_dir() {
            std::fs::remove_dir_all(&p).with_context(|| format!("rm {}", p.display()))?;
        }
    }
    let tsbuild = dir.join("tsconfig.tsbuildinfo");
    if tsbuild.is_file() {
        std::fs::remove_file(&tsbuild)?;
    }
    Ok(())
}
