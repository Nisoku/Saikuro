use std::collections::BTreeSet;
use std::path::Path;
use std::time::Instant;

use anyhow::Context;

use crate::paths;
use crate::run;

#[derive(Debug, Clone)]
pub struct Combo {
    pub name: &'static str,
    pub target: Option<&'static str>,
    pub cargo_args: &'static [&'static str],
    pub notes: &'static str,
}

pub const MATRIX: &[Combo] = &[
    Combo {
        name: "native (host, std)",
        target: None,
        cargo_args: &[],
        notes: "default features",
    },
    Combo {
        name: "native (ws)",
        target: None,
        cargo_args: &["--features", "ws"],
        notes: "default features + websocket transport",
    },
    Combo {
        name: "wasm (std)",
        target: Some("wasm32-unknown-unknown"),
        cargo_args: &["--no-default-features", "--features", "std,wasm"],
        notes: "std wasm",
    },
    Combo {
        name: "wasm (no_std)",
        target: Some("wasm32-unknown-unknown"),
        cargo_args: &["--no-default-features", "--features", "wasm"],
        notes: "no_std wasm",
    },
    Combo {
        name: "embedded (no_std) (Cortex-M3 / RP2350-class)",
        target: Some("thumbv7m-none-eabi"),
        cargo_args: &["--no-default-features", "--features", "embedded,tcp-net"],
        notes: "no_std embedded",
    },
    Combo {
        name: "embedded (no_std) (Cortex-M0+ / RP2040)",
        target: Some("thumbv6m-none-eabi"),
        cargo_args: &["--no-default-features", "--features", "embedded,tcp-net"],
        notes: "no_std embedded",
    },
    Combo {
        name: "embedded (no_std) (Cortex-M33 / RP2350)",
        target: Some("thumbv8m.main-none-eabihf"),
        cargo_args: &["--no-default-features", "--features", "embedded,tcp-net"],
        notes: "no_std embedded",
    },
    Combo {
        name: "embedded (no_std) (AArch64 bare-metal)",
        target: Some("aarch64-unknown-none"),
        cargo_args: &["--no-default-features", "--features", "embedded,tcp-net"],
        notes: "no_std embedded",
    },
    Combo {
        name: "embedded (no_std) (RISC-V 32 IMAC)",
        target: Some("riscv32imac-unknown-none-elf"),
        cargo_args: &["--no-default-features", "--features", "embedded,tcp-net"],
        notes: "no_std embedded",
    },
    Combo {
        name: "embedded (no_std) (RISC-V 32 IMC / ESP32-C3)",
        target: Some("riscv32imc-unknown-none-elf"),
        cargo_args: &["--no-default-features", "--features", "embedded,tcp-net"],
        notes: "no_std embedded",
    },
    Combo {
        name: "wasi preview1 (no_std)",
        target: Some("wasm32-wasip1"),
        cargo_args: &[
            "--no-default-features",
            "--features",
            "no_std,wasi-tcp,wasi-host,wasi-preview1,default-panic-handler",
        ],
        notes: "no_std wasi (preview1)",
    },
    Combo {
        name: "wasi preview2 (no_std)",
        target: Some("wasm32-wasip2"),
        cargo_args: &[
            "--no-default-features",
            "--features",
            "no_std,wasi-tcp,wasi-host,wasi-preview2",
        ],
        notes: "no_std wasi (preview2)",
    },
    Combo {
        name: "wasi preview1 (ws)",
        target: Some("wasm32-wasip1"),
        cargo_args: &[
            "--no-default-features",
            "--features",
            "no_std,wasi-tcp,wasi-host,wasi-preview1,ws-wasi,default-panic-handler",
        ],
        notes: "no_std wasi websocket client (preview1)",
    },
    Combo {
        name: "wasi preview2 (ws)",
        target: Some("wasm32-wasip2"),
        cargo_args: &[
            "--no-default-features",
            "--features",
            "no_std,wasi-tcp,wasi-host,wasi-preview2,ws-wasi,default-panic-handler",
        ],
        notes: "no_std wasi websocket client (preview2)",
    },
    Combo {
        name: "wasi preview1 (std)",
        target: Some("wasm32-wasip1"),
        cargo_args: &[
            "--no-default-features",
            "--features",
            "std,no_std,wasi-tcp,wasi-host,wasi-preview1",
        ],
        notes: "std wasi (preview1)",
    },
    Combo {
        name: "wasi preview2 (std)",
        target: Some("wasm32-wasip2"),
        cargo_args: &[
            "--no-default-features",
            "--features",
            "std,no_std,wasi-tcp,wasi-host,wasi-preview2",
        ],
        notes: "std wasi (preview2)",
    },
    Combo {
        name: "wasi preview1 (std ws)",
        target: Some("wasm32-wasip1"),
        cargo_args: &[
            "--no-default-features",
            "--features",
            "std,no_std,wasi-tcp,wasi-host,wasi-preview1,ws-wasi",
        ],
        notes: "std wasi websocket client (preview1)",
    },
    Combo {
        name: "wasi preview2 (std ws)",
        target: Some("wasm32-wasip2"),
        cargo_args: &[
            "--no-default-features",
            "--features",
            "std,no_std,wasi-tcp,wasi-host,wasi-preview2,ws-wasi",
        ],
        notes: "std wasi websocket client (preview2)",
    },
];

#[derive(Debug, Clone)]
pub struct ReportEntry {
    pub crate_name: String,
    pub name: String,
    pub target: String,
    pub features: String,
    pub notes: String,
    pub passed: bool,
    pub returncode: i32,
    pub errors: usize,
    pub warnings: usize,
    pub diagnostics: Vec<String>,
    pub seconds: f64,
}

fn workspace_feature_sets(
    root: &Path,
) -> anyhow::Result<std::collections::HashMap<String, BTreeSet<String>>> {
    let output = run::run_capture(
        root,
        "cargo",
        [
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
            paths::engine_manifest().display().to_string().as_str(),
        ],
    )
    .context("cargo metadata")?;
    let meta: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("parse metadata")?;
    let mut map = std::collections::HashMap::new();
    for pkg in meta["packages"].as_array().into_iter().flatten() {
        let name = pkg["name"].as_str().unwrap_or_default().to_string();
        let feats = pkg["features"]
            .as_object()
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default();
        map.insert(name, feats);
    }
    Ok(map)
}

fn check_combo(
    root: &Path,
    crate_name: &str,
    combo: &Combo,
    declared: Option<&BTreeSet<String>>,
) -> anyhow::Result<ReportEntry> {
    let mut args: Vec<String> = vec![
        "check".into(),
        "-p".into(),
        crate_name.into(),
        "--lib".into(),
        "--manifest-path".into(),
        paths::engine_manifest().display().to_string(),
    ];
    if let Some(target) = combo.target {
        args.push("--target".into());
        args.push(target.into());
    }

    let mut cargo_args: Vec<String> = combo.cargo_args.iter().map(|s| s.to_string()).collect();
    if let (Some(declared), Some(fi)) =
        (declared, cargo_args.iter().position(|a| a == "--features"))
    {
        let kept: Vec<String> = cargo_args[fi + 1]
            .split(',')
            .filter(|f| declared.contains(*f))
            .map(str::to_string)
            .collect();
        if kept.is_empty() {
            let features = cargo_args.join(" ");
            return Ok(ReportEntry {
                crate_name: crate_name.into(),
                name: combo.name.into(),
                target: combo.target.unwrap_or("<host>").into(),
                features: if features.is_empty() {
                    "<default>".into()
                } else {
                    features
                },
                notes: format!("{} (skipped: no matching features)", combo.notes),
                passed: true,
                returncode: 0,
                errors: 0,
                warnings: 0,
                diagnostics: vec![],
                seconds: 0.0,
            });
        }
        cargo_args[fi + 1] = kept.join(",");
    }
    args.extend(cargo_args.iter().cloned());

    let start = Instant::now();
    let output = run::run_capture(root, "cargo", &args)
        .with_context(|| format!("spawn cargo check for {crate_name} :: {}", combo.name))?;
    let seconds = start.elapsed().as_secs_f64();

    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    let errors = text.lines().filter(|l| l.starts_with("error")).count();
    let warnings = text
        .lines()
        .filter(|l| l.starts_with("warning") && !l.contains("generated"))
        .count();
    let diagnostics = extract_diagnostics(&text);
    let passed = output.status.success() && errors == 0;

    Ok(ReportEntry {
        crate_name: crate_name.into(),
        name: combo.name.into(),
        target: combo.target.unwrap_or("<host>").into(),
        features: cargo_args.join(" "),
        notes: combo.notes.into(),
        passed,
        returncode: output.status.code().unwrap_or(-1),
        errors,
        warnings,
        diagnostics,
        seconds: (seconds * 10.0).round() / 10.0,
    })
}

fn extract_diagnostics(text: &str) -> Vec<String> {
    let mut diags = Vec::new();
    let mut in_diag = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("error")
            || line.starts_with("warning")
            || line.starts_with("note:")
            || line.starts_with("help:")
            || line.starts_with("-->")
        {
            in_diag = true;
            diags.push(raw.to_string());
        } else if in_diag {
            if line.is_empty() {
                in_diag = false;
            } else if line.starts_with('|')
                || line.starts_with('=')
                || line.starts_with('^')
                || line.starts_with('*')
                || raw.starts_with([' ', '\t'])
                || line.starts_with("error[")
            {
                diags.push(raw.to_string());
            } else {
                in_diag = false;
            }
        }
    }
    diags
}

/// Which crates to check. `all` includes every workspace member.
pub fn select_crates(
    root: &Path,
    all: bool,
    crate_name: Option<&str>,
) -> anyhow::Result<Vec<String>> {
    if let Some(name) = crate_name {
        return Ok(vec![name.to_string()]);
    }
    if all {
        let output = run::run_capture(
            root,
            "cargo",
            [
                "metadata",
                "--no-deps",
                "--format-version",
                "1",
                "--manifest-path",
                paths::engine_manifest().display().to_string().as_str(),
            ],
        )
        .context("cargo metadata")?;
        let meta: serde_json::Value =
            serde_json::from_slice(&output.stdout).context("parse metadata")?;
        let mut names: Vec<String> = meta["packages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p["name"].as_str().map(str::to_string))
            .collect();
        names.sort();
        return Ok(names);
    }
    Ok(vec!["saikuro-runtime".to_string()])
}

pub fn run_matrix(
    root: &Path,
    all: bool,
    crate_name: Option<&str>,
    json: Option<&Path>,
) -> anyhow::Result<()> {
    let crates = select_crates(root, all, crate_name)?;
    let features = workspace_feature_sets(root)?;

    println!("cargo : cargo");
    println!("root  : {}", root.display());
    println!(
        "matrix: {} combos x {} crate(s)\n",
        MATRIX.len(),
        crates.len()
    );

    let mut results: Vec<ReportEntry> = Vec::new();
    for crate_name in &crates {
        println!("=== crate: {crate_name} ===");
        let declared = features.get(crate_name);
        for combo in MATRIX {
            let entry = check_combo(root, crate_name, combo, declared)?;
            let status = if entry.passed { "PASS" } else { "FAIL" };
            println!(
                "  [{status}] {:<22} target={:<20} errs={:<3} warns={:<3} {}s",
                entry.name, entry.target, entry.errors, entry.warnings, entry.seconds
            );
            results.push(entry);
        }
    }

    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let warn_total: usize = results.iter().map(|r| r.warnings).sum();
    let err_total: usize = results.iter().map(|r| r.errors).sum();
    println!(
        "\n=== SUMMARY: {passed}/{total} passed ({warn_total} warnings, {err_total} errors) ==="
    );

    let noisy: Vec<&ReportEntry> = results
        .iter()
        .filter(|r| r.warnings > 0 || r.errors > 0)
        .collect();
    if !noisy.is_empty() {
        println!("\n=== WARNINGS & ERRORS ===");
        for r in noisy {
            let tag = if r.passed { "WARN" } else { "FAIL" };
            println!(
                "\n[{tag}] {} :: {} (target={}, features={})",
                r.crate_name, r.name, r.target, r.features
            );
            if r.diagnostics.is_empty() {
                println!("  (no diagnostic lines captured)");
            } else {
                for line in &r.diagnostics {
                    println!("  {line}");
                }
            }
        }
    }

    if let Some(path) = json {
        let entries: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "crate": r.crate_name,
                    "name": r.name,
                    "target": r.target,
                    "features": r.features,
                    "notes": r.notes,
                    "passed": r.passed,
                    "returncode": r.returncode,
                    "errors": r.errors,
                    "warnings": r.warnings,
                    "diags": r.diagnostics,
                    "seconds": r.seconds,
                })
            })
            .collect();
        let body = serde_json::to_string_pretty(&entries)?;
        std::fs::write(path, body).with_context(|| format!("write {}", path.display()))?;
        println!("\nwrote {}", path.display());
    }

    if passed == total {
        Ok(())
    } else {
        anyhow::bail!("{passed}/{total} matrix combos passed")
    }
}
