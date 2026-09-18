use std::path::Path;
use std::process::Command;

use anyhow::Context;

use crate::config::{Config, Source};
use crate::gates::sha256_hex;
use crate::paths;
use crate::run;

const BINSTALL_INSTALL_URL: &str = concat!(
    "https://raw.githubusercontent.com/cargo-bins/cargo-binstall/",
    "9d36bebae244fb1a7fd7831d3c5a4bc8f1cab230/install-from-binstall-release.sh"
);
const BINSTALL_INSTALL_SHA256: &str =
    "d3a93702160e0ec03e2a4e996855db1f01adee801fb84a43add24e0877ef8eae";
const BINSTALL_VERSION: &str = "v1.23.0";

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
    if let Some(expected) = &tool.version {
        let out = run::run_capture(Path::new("/"), &tool.bin, ["--version"])?;
        let installed = format!(
            "{} {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        if !out.status.success() || !installed.split_whitespace().any(|part| part == expected) {
            println!("MISMATCH {:23} (expected {})", tool.name, expected);
            return Ok(false);
        }
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

// Removes the verified bootstrap script from the temp dir on scope exit.
struct TempScript(std::path::PathBuf);

impl Drop for TempScript {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn ensure_binstall(check_only: bool) -> anyhow::Result<()> {
    if run::which("cargo-binstall") {
        return Ok(());
    }
    if check_only {
        anyhow::bail!("cargo-binstall is missing; run `cargo xtask setup`");
    }
    println!("installing cargo-binstall...");
    let out = run::run_capture(Path::new("/"), "curl", ["-fLsS", BINSTALL_INSTALL_URL])
        .context("download cargo-binstall bootstrap")?;
    if !out.status.success() {
        anyhow::bail!(
            "download cargo-binstall bootstrap failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let digest = sha256_hex(&out.stdout);
    if digest != BINSTALL_INSTALL_SHA256 {
        anyhow::bail!(
            "cargo-binstall bootstrap digest mismatch (got {digest}, want {BINSTALL_INSTALL_SHA256}); \
             refusing to execute"
        );
    }
    let script = TempScript(std::env::temp_dir().join(format!(
        "cargo-binstall-bootstrap-{}.sh",
        std::process::id()
    )));
    std::fs::write(&script.0, &out.stdout)
        .with_context(|| format!("write {}", script.0.display()))?;
    let status = Command::new("sh")
        .arg(&script.0)
        .env("BINSTALL_VERSION", BINSTALL_VERSION)
        .current_dir(Path::new("/"))
        .status()
        .with_context(|| "run cargo-binstall bootstrap")?;
    if !status.success() {
        anyhow::bail!("cargo-binstall bootstrap exited with {status}");
    }
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
