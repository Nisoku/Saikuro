// csharp

use anyhow::Context;
use std::path::PathBuf;

use super::{run_fix_step, run_formatter};
use crate::{paths, run};

pub(super) fn cs_dir() -> PathBuf {
    paths::adapters_dir().join("csharp").join("Saikuro")
}

pub(super) fn cs_src() -> PathBuf {
    cs_dir().join("src").join("Saikuro.csproj")
}

pub(super) fn cs_test() -> PathBuf {
    cs_dir().join("tests").join("Saikuro.Tests.csproj")
}

fn csharp_format_check() -> anyhow::Result<()> {
    let project = cs_src();
    run_fix_step(
        "C#",
        &cs_dir(),
        "dotnet",
        &[
            "format",
            project.display().to_string().as_str(),
            "--verify-no-changes",
        ],
        &["format", project.display().to_string().as_str()],
    )
}

pub(super) fn csharp_format() -> anyhow::Result<()> {
    let project = cs_src();
    run_formatter(
        "C#",
        &cs_dir(),
        "dotnet",
        &["format", project.display().to_string().as_str()],
    )
}

pub(super) fn csharp_check() -> anyhow::Result<()> {
    csharp_format_check()?;
    let src = cs_src().display().to_string();
    let test = cs_test().display().to_string();
    run::run(
        &cs_dir(),
        "dotnet",
        ["build", src.as_str(), "-c", "Release"],
    )?;
    run::run(
        &cs_dir(),
        "dotnet",
        ["test", test.as_str(), "-c", "Release"],
    )
}

pub(super) fn csharp_clean() -> anyhow::Result<()> {
    for sub in ["src", "tests"] {
        for kind in ["bin", "obj"] {
            let p = cs_dir().join(sub).join(kind);
            if p.is_dir() {
                std::fs::remove_dir_all(&p).with_context(|| format!("rm {}", p.display()))?;
            }
        }
    }
    Ok(())
}
