use std::path::Path;

use anyhow::Context;

use crate::config::{Config, Source};
use crate::paths;
use crate::run;

fn verify(tool: &crate::config::Tool) -> anyhow::Result<bool> {
    if !run::which(&tool.bin) {
        println!(
            "MISSING {:24} ({}:{})",
            tool.name,
            tool.pkg.as_deref().unwrap_or("-"),
            tool.version.as_deref().unwrap_or("-")
        );
        return Ok(false);
    }
    let mut extra_ok = true;
    for extra in &tool.also {
        if !run::which(extra) {
            println!("MISSING {:24} (companion binary)", extra);
            extra_ok = false;
        }
    }
    if extra_ok {
        println!("ok      {}", tool.name);
    }
    Ok(extra_ok)
}

fn ensure_rust(manifest: &Config) -> anyhow::Result<()> {
    let toolchain = &manifest.rust.toolchain;
    let out = run::run_capture(Path::new("/"), "rustup", ["toolchain", "list"])?;
    let installed: Vec<String> = std::str::from_utf8(&out.stdout)?
        .lines()
        .map(|l| l.trim().to_string())
        .collect();
    let present = installed
        .iter()
        .any(|l| l.starts_with(&format!("{toolchain}-")) || l.starts_with(toolchain));
    if !present {
        run::run(
            Path::new("/"),
            "rustup",
            [
                "toolchain",
                "install",
                toolchain,
                "--profile",
                "minimal",
                "--component",
                "rustfmt",
                "--component",
                "clippy",
            ],
        )
        .with_context(|| format!("install rust {toolchain}"))?;
    } else {
        println!("ok      rust {}", toolchain);
    }
    let nightly = &manifest.rust.nightly_channel;
    if !installed.iter().any(|l| l.starts_with(nightly)) {
        run::run(
            Path::new("/"),
            "rustup",
            [
                "toolchain",
                "install",
                nightly,
                "--profile",
                "minimal",
                "--component",
                "rust-src",
            ],
        )
        .with_context(|| format!("install {nightly}"))?;
    } else {
        println!("ok      rust {nightly}");
    }
    Ok(())
}

const BINSTALL_INSTALL_URL: &str = "https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh";

fn ensure_binstall(check_only: bool) -> anyhow::Result<()> {
    if run::which("cargo-binstall") {
        return Ok(());
    }
    if check_only {
        anyhow::bail!("cargo-binstall is missing; run `cargo xtask setup`");
    }
    println!("installing cargo-binstall...");
    let script = format!("curl -LsSf {BINSTALL_INSTALL_URL} | bash");
    run::run(Path::new("/"), "sh", ["-c", script.as_str()]).context("bootstrap cargo-binstall")?;
    if !run::which("cargo-binstall") {
        anyhow::bail!("cargo-binstall installed but not found on PATH");
    }
    Ok(())
}

pub fn sync(check_only: bool) -> anyhow::Result<()> {
    let manifest = Config::load(&paths::tools_manifest())?;
    ensure_rust(&manifest)?;
    ensure_binstall(check_only)?;

    let mut failed = Vec::new();
    let mut pending_system = Vec::new();
    for tool in &manifest.tools {
        match tool.source {
            Source::System => {
                if !verify(tool)? {
                    if check_only {
                        failed.push(format!("{} (system)", tool.name));
                    } else {
                        pending_system.push(tool.name.to_string());
                    }
                }
            }
            Source::Binstall => {
                if verify(tool)? {
                    continue;
                }
                if check_only {
                    failed.push(tool.name.clone());
                    continue;
                }
                let pkg = tool.pkg.as_deref().expect("binstall tool has a pkg");
                let version = tool.version.as_deref().expect("binstall tool is pinned");
                let versioned = format!("{pkg}@{version}");
                println!("installing {pkg} {version}...");
                if let Err(e) = run::run(
                    Path::new("/"),
                    "cargo",
                    ["binstall", "-y", "--locked", versioned.as_str()],
                )
                .with_context(|| format!("cargo binstall {pkg}"))
                {
                    println!("FAILED  {pkg}: {e:#}");
                    failed.push(format!("{pkg} (binstall)"));
                }
            }
        }
    }

    for name in &pending_system {
        println!(
            "WARN    {name} (system) is missing; install it via your environment \
             (the commands that need it will fail at point of use otherwise)"
        );
    }

    if !failed.is_empty() {
        anyhow::bail!(
            "missing tools: {}. Run `cargo xtask setup` to install (or install them \
             via your package manager).",
            failed.join(", ")
        );
    }
    println!(
        "\ntoolchain synced against {}",
        paths::tools_manifest().display()
    );
    Ok(())
}

pub fn list() -> anyhow::Result<()> {
    let manifest = Config::load(&paths::tools_manifest())?;
    println!("rust     {}", manifest.rust.toolchain);
    println!("nightly  {}", manifest.rust.nightly_channel);
    for tool in &manifest.tools {
        println!(
            "{:12} {} ({})",
            tool.name,
            tool.version.as_deref().unwrap_or("host"),
            tool.source.describe()
        );
    }
    Ok(())
}
