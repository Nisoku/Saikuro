use std::io::Write;

use anyhow::Context;

use crate::paths;
use crate::run;

fn root() -> std::path::PathBuf {
    paths::repo_root()
}

fn require(tool: &str) -> anyhow::Result<()> {
    if run::which(tool) {
        Ok(())
    } else {
        anyhow::bail!("{tool} is not installed. Run `cargo xtask setup` to sync Tools/tools.toml.")
    }
}

pub fn deny() -> anyhow::Result<()> {
    require("cargo-deny")?;
    let manifest = paths::engine_manifest().display().to_string();
    let config = paths::deny_config().display().to_string();
    run::run(
        &root(),
        "cargo",
        [
            "deny",
            "--manifest-path",
            manifest.as_str(),
            "--config",
            config.as_str(),
            "check",
        ],
    )
    .with_context(|| "cargo deny check")
}

pub fn audit() -> anyhow::Result<()> {
    require("cargo-audit")?;
    run::run(&root(), "cargo", ["audit"]).with_context(|| "cargo audit")
}

pub fn miri() -> anyhow::Result<()> {
    let status = std::process::Command::new("cargo")
        .args([
            "+nightly",
            "miri",
            "run",
            "-p",
            "saikuro-tests",
            "--no-default-features",
            "--features",
            "embedded",
            "--bin",
            "embedded-host",
        ])
        .current_dir(root())
        .env("MIRIFLAGS", "-Zmiri-strict-provenance")
        .status()
        .context("spawn miri")?;
    if !status.success() {
        anyhow::bail!("miri exited with {status}");
    }
    Ok(())
}

pub fn typos() -> anyhow::Result<()> {
    require("typos")?;
    run::run(&root(), "typos", &[] as &[&str]).with_context(|| "typos")
}

const GEIGER_BASELINE: &str = "geiger.baseline";

fn normalize_report(text: &str) -> String {
    let prefix = root().display().to_string();
    let mut out = String::new();
    for line in text.lines() {
        if line.contains("rustc version") || line.trim().starts_with("Scanning") {
            continue;
        }
        out.push_str(&line.replace(&prefix, "."));
        out.push('\n');
    }
    out
}

fn sha256_hex(input: &str) -> String {
    use sha2::Digest;
    use sha2::Sha256;
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let out = hasher.finalize();
    out.iter().map(|b| format!("{b:02x}")).collect()
}

fn baseline_path(name: &str) -> std::path::PathBuf {
    paths::baselines_dir().join(name)
}

const ENGINE_FEATURES: [&str; 4] = ["native", "no_std", "wasm", "embedded"];

struct GeigerMember {
    name: String,
    manifest_path: std::path::PathBuf,
    required_features: Vec<String>,
}

fn workspace_members() -> anyhow::Result<Vec<GeigerMember>> {
    use serde_json::Value;
    let out = run::run_capture(&root(), "cargo", ["metadata", "--format-version", "1"])
        .with_context(|| "cargo metadata")?;
    if !out.status.success() {
        anyhow::bail!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let json: Value = serde_json::from_slice(&out.stdout).context("parse cargo metadata")?;
    let workspace_members = json
        .get("workspace_members")
        .and_then(Value::as_array)
        .context("metadata has no workspace_members")?;
    let packages = json
        .get("packages")
        .and_then(Value::as_array)
        .context("metadata has no packages")?;
    let mut members = Vec::new();
    for id in workspace_members {
        let id = id.as_str().context("workspace member id not a string")?;
        let pkg = packages
            .iter()
            .find(|p| p.get("id").and_then(Value::as_str) == Some(id))
            .context("workspace member missing from packages")?;
        let name = pkg
            .get("name")
            .and_then(Value::as_str)
            .context("package has no name")?
            .to_string();
        let manifest_path = pkg
            .get("manifest_path")
            .and_then(Value::as_str)
            .context("package has no manifest_path")?
            .to_string();
        let mut required_features = std::collections::BTreeSet::new();
        for target in pkg
            .get("targets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(reqs) = target.get("required-features").and_then(Value::as_array) {
                for req in reqs.iter().filter_map(Value::as_str) {
                    required_features.insert(req.to_string());
                }
            }
        }
        members.push(GeigerMember {
            name,
            manifest_path: std::path::PathBuf::from(manifest_path),
            required_features: required_features.into_iter().collect(),
        });
    }
    members.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(members)
}

pub fn geiger(update: bool) -> anyhow::Result<()> {
    require("cargo-geiger")?;
    let mut combined = String::new();
    for member in workspace_members()? {
        let engines: Vec<&str> = ENGINE_FEATURES
            .iter()
            .copied()
            .filter(|engine| member.required_features.iter().any(|f| f == engine))
            .collect();
        if engines.len() > 1 {
            println!(
                "geiger: skipping {} (targets require engines {engines:?})",
                member.name
            );
            continue;
        }
        print!("geiger: scanning {} ... ", member.name);
        std::io::stdout().flush().ok();
        let manifest = member.manifest_path.display().to_string();
        let out = run::run_capture(
            &root(),
            "cargo",
            &[
                "geiger",
                "--output-format",
                "Json",
                "--manifest-path",
                manifest.as_str(),
            ],
        )
        .with_context(|| format!("geiger run for {}", member.name))?;
        if !out.status.success() {
            anyhow::bail!(
                "geiger exited with {status} for {name}",
                status = out.status,
                name = member.name
            );
        }
        combined.push_str(&String::from_utf8_lossy(&out.stdout));
        combined.push('\n');
        println!("done");
    }
    let digest = sha256_hex(&normalize_report(&combined));
    let baseline = baseline_path(GEIGER_BASELINE);
    if update {
        if let Some(dir) = baseline.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&baseline, &digest)
            .with_context(|| format!("write {}", baseline.display()))?;
        println!("geiger: baseline written to {}", baseline.display());
        return Ok(());
    }
    if !baseline.is_file() {
        anyhow::bail!("geiger baseline missing. Run `cargo xtask geiger --update` to record one.");
    }
    let expected = std::fs::read_to_string(&baseline)
        .with_context(|| format!("read {}", baseline.display()))?
        .trim()
        .to_string();
    if digest != expected {
        anyhow::bail!(
            "geiger digest drifted from {} (see {}). Re-run `cargo xtask geiger --update` \
             only after reviewing the diff.",
            baseline.display(),
            baseline.display()
        );
    }
    println!("geiger: ok (baseline {GEIGER_BASELINE})");
    Ok(())
}

// TODO: Disabled until kbknapp/cargo-outdated#122 merges
// pub fn outdated() -> anyhow::Result<()> {
//     require("cargo-outdated")?;
//     run::run(&root(), "cargo", ["outdated", "--exit-code", "1"]).with_context(|| "cargo outdated")
// }

pub fn spellcheck() -> anyhow::Result<()> {
    require("cargo-spellcheck")?;
    run::run(
        &root(),
        "cargo",
        ["spellcheck", "-r", "-m", "1", "check", "Build"],
    )
    .with_context(|| "cargo spellcheck")
}

pub fn deadlinks() -> anyhow::Result<()> {
    let out = run::run_capture(&root(), "cargo", ["+nightly", "doc", "--workspace", "--no-deps"])
        .with_context(|| "cargo doc")?;
    let combined = String::from_utf8_lossy(&out.stdout);
    let combined_stderr = String::from_utf8_lossy(&out.stderr);
    let mut problems = Vec::new();
    for (stream, text) in [("stderr", combined_stderr.as_ref()), ("stdout", combined.as_ref())] {
        for line in text.lines() {
            if ["unresolved link", "unresolved intra-doc link", "links to private item"]
                .iter()
                .any(|marker| line.contains(marker))
            {
                problems.push(format!("[{stream}] {line}"));
            }
        }
    }
    if !problems.is_empty() {
        anyhow::bail!(
            "broken intra-doc links:\n{}",
            problems.join("\n")
        );
    }
    println!("deadlinks: ok (rustdoc intra-doc links clean)");
    Ok(())
}
