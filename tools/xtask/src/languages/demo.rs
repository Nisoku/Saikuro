// demo

use std::path::{Path, PathBuf};

use anyhow::Context;

use super::{collect_recursive, run_fix_step, run_formatter};
use crate::{paths, run};

pub(super) fn demo_dir() -> PathBuf {
    paths::demo_dir()
}

fn wasm_dir() -> PathBuf {
    paths::demo_wasm_dir()
}

/// Collect sources under `root`, skipping build outputs and vendored trees.
fn collect_sources(root: &Path, patterns: &[&str]) -> Vec<PathBuf> {
    collect_recursive(root, patterns)
        .into_iter()
        .filter(|p| {
            !p.components().any(|c| {
                matches!(
                    c.as_os_str().to_str(),
                    Some("target" | "build" | "node_modules" | "bin" | "obj")
                )
            })
        })
        .collect()
}

fn rust_manifests() -> Vec<PathBuf> {
    ["c", "cpp", "runtime", "rust"]
        .iter()
        .map(|name| wasm_dir().join(name).join("Cargo.toml"))
        .filter(|manifest| manifest.is_file())
        .collect()
}

fn cs_dir() -> PathBuf {
    wasm_dir().join("csharp").join("InsightLab")
}

fn root() -> PathBuf {
    paths::repo_root()
}

/// Frontend formatter/linter over the Vite project (prettier + tsc + vite).
fn frontend_format_check() -> anyhow::Result<()> {
    run_fix_step(
        "Demo frontend",
        &demo_dir(),
        "npm",
        &["run", "format:check"],
        &["run", "format"],
    )
}

fn frontend_format() -> anyhow::Result<()> {
    run_formatter("Demo frontend", &demo_dir(), "npm", &["run", "format"])
}

fn clang_check(label: &str, sub: &str, patterns: &[&str]) -> anyhow::Result<()> {
    if !run::which("clang-format") {
        println!("[WARN] clang-format not found; skipping {label} format check");
        return Ok(());
    }
    let sources = collect_sources(&wasm_dir().join(sub), patterns);
    if sources.is_empty() {
        return Ok(());
    }
    let files: Vec<String> = sources.iter().map(|p| p.display().to_string()).collect();
    let mut check = vec!["--dry-run".to_string(), "-Werror".to_string()];
    check.extend(files.iter().cloned());
    let mut fix = vec!["-i".to_string()];
    fix.extend(files.iter().cloned());
    run_fix_step(
        label,
        &wasm_dir(),
        "clang-format",
        &check.iter().map(String::as_str).collect::<Vec<_>>(),
        &fix.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

fn clang_format(label: &str, sub: &str, patterns: &[&str]) -> anyhow::Result<()> {
    if !run::which("clang-format") {
        println!("[WARN] clang-format not found; skipping {label} format");
        return Ok(());
    }
    let sources = collect_sources(&wasm_dir().join(sub), patterns);
    if sources.is_empty() {
        return Ok(());
    }
    let mut args = vec!["-i".to_string()];
    args.extend(sources.iter().map(|p| p.display().to_string()));
    run::run(
        &wasm_dir(),
        "clang-format",
        args.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

fn py_sources() -> Vec<PathBuf> {
    collect_sources(&wasm_dir().join("python"), &["*.py"])
}

fn python_args(prefix: &[&str]) -> Vec<String> {
    let mut args: Vec<String> = prefix.iter().map(|s| s.to_string()).collect();
    args.extend(py_sources().iter().map(|p| p.display().to_string()));
    args
}

fn python_format_check() -> anyhow::Result<()> {
    if py_sources().is_empty() {
        return Ok(());
    }
    let check = python_args(&["format", "--check"]);
    let fix = python_args(&["format"]);
    run_fix_step(
        "Demo Python",
        &demo_dir(),
        "ruff",
        &check.iter().map(String::as_str).collect::<Vec<_>>(),
        &fix.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

fn python_format() -> anyhow::Result<()> {
    if py_sources().is_empty() {
        return Ok(());
    }
    let args = python_args(&["format"]);
    run_formatter(
        "Demo Python",
        &demo_dir(),
        "ruff",
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

fn python_lint() -> anyhow::Result<()> {
    if py_sources().is_empty() {
        return Ok(());
    }
    let args = python_args(&["check"]);
    run::run(
        &demo_dir(),
        "ruff",
        args.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

fn rust_wasm_format_check() -> anyhow::Result<()> {
    for manifest in rust_manifests() {
        let path = manifest.display().to_string();
        run_fix_step(
            "Demo Rust",
            &demo_dir(),
            "cargo",
            &["fmt", "--manifest-path", path.as_str(), "--", "--check"],
            &["fmt", "--manifest-path", path.as_str()],
        )?;
    }
    Ok(())
}

fn rust_wasm_format() -> anyhow::Result<()> {
    for manifest in rust_manifests() {
        let path = manifest.display().to_string();
        run_formatter(
            "Demo Rust",
            &demo_dir(),
            "cargo",
            &["fmt", "--manifest-path", path.as_str()],
        )?;
    }
    Ok(())
}

fn csharp_format_check() -> anyhow::Result<()> {
    let project = cs_dir().join("InsightLab.csproj").display().to_string();
    run_fix_step(
        "Demo C#",
        &cs_dir(),
        "dotnet",
        &["format", project.as_str(), "--verify-no-changes"],
        &["format", project.as_str()],
    )
}

fn csharp_format() -> anyhow::Result<()> {
    let project = cs_dir().join("InsightLab.csproj").display().to_string();
    run_formatter(
        "Demo C#",
        &cs_dir(),
        "dotnet",
        &["format", project.as_str()],
    )
}

pub(super) fn demo_format() -> anyhow::Result<()> {
    frontend_format()?;
    clang_format("Demo C", "c", &["*.c", "*.h"])?;
    clang_format("Demo C++", "cpp", &["*.cpp", "*.hpp", "*.h"])?;
    python_format()?;
    rust_wasm_format()?;
    csharp_format()
}

pub(super) fn demo_format_check() -> anyhow::Result<()> {
    frontend_format_check()?;
    clang_check("Demo C", "c", &["*.c", "*.h"])?;
    clang_check("Demo C++", "cpp", &["*.cpp", "*.hpp", "*.h"])?;
    python_format_check()?;
    rust_wasm_format_check()?;
    csharp_format_check()
}

pub(super) fn demo_lint() -> anyhow::Result<()> {
    python_lint()
}

pub(super) fn demo_check() -> anyhow::Result<()> {
    demo_format_check()?;
    demo_lint()?;
    run::run(&demo_dir(), "npm", ["run", "typecheck"])?;
    run::run(&demo_dir(), "npm", ["run", "build"])
}

pub(super) fn demo_setup() -> anyhow::Result<()> {
    run::run(&demo_dir(), "npm", ["install"])
}

pub(super) fn demo_clean() -> anyhow::Result<()> {
    for path in [
        demo_dir().join("node_modules"),
        demo_dir().join("dist"),
        paths::public_wasm_dir(),
        cs_dir().join("bin"),
        cs_dir().join("obj"),
    ] {
        if path.is_dir() {
            std::fs::remove_dir_all(&path).with_context(|| format!("rm {}", path.display()))?;
        }
    }
    Ok(())
}

/// Directories that must exist before any component build writes into them.
fn ensure_public_dirs() -> anyhow::Result<()> {
    for lang in ["c", "cpp", "runtime", "rust", "python", "csharp"] {
        std::fs::create_dir_all(paths::public_wasm_dir().join(lang))
            .with_context(|| format!("mkdir {}", paths::public_wasm_dir().join(lang).display()))?;
    }
    Ok(())
}

/// Build a wasm-pack component into `Demo/public/wasm/<out>`.
fn wasm_pack(src: &Path, out: &str) -> anyhow::Result<()> {
    let out_dir = paths::public_wasm_dir().join(out);
    std::fs::create_dir_all(&out_dir).with_context(|| format!("mkdir {}", out_dir.display()))?;
    run::run(
        &root(),
        "wasm-pack",
        [
            "build",
            src.display().to_string().as_str(),
            "--target",
            "web",
            "--out-dir",
            out_dir.display().to_string().as_str(),
            "--release",
        ],
    )
    .with_context(|| format!("wasm-pack {out}"))?;
    let gitignore = out_dir.join(".gitignore");
    if gitignore.is_file() {
        std::fs::remove_file(&gitignore)
            .with_context(|| format!("remove {}", gitignore.display()))?;
    }
    Ok(())
}

pub(super) fn demo_wasm_runtime() -> anyhow::Result<()> {
    wasm_pack(&wasm_dir().join("runtime"), "runtime")
}

pub(super) fn demo_wasm_rust() -> anyhow::Result<()> {
    wasm_pack(&wasm_dir().join("rust"), "rust")
}

pub(super) fn demo_wasm_rust_all() -> anyhow::Result<()> {
    demo_wasm_runtime()?;
    demo_wasm_rust()
}

pub(super) fn demo_wasm_c() -> anyhow::Result<()> {
    wasm_pack(&wasm_dir().join("c"), "c")
}

pub(super) fn demo_wasm_cpp() -> anyhow::Result<()> {
    wasm_pack(&wasm_dir().join("cpp"), "cpp")
}

pub(super) fn demo_wasm_csharp() -> anyhow::Result<()> {
    let project = cs_dir().join("InsightLab.csproj");
    if !project.is_file() {
        anyhow::bail!("C# demo project not found: {}", project.display());
    }
    run::run(
        &root(),
        "dotnet",
        [
            "publish",
            &project.display().to_string(),
            "-c",
            "Release",
            "-p:RuntimeIdentifier=browser-wasm",
            "-p:SelfContained=true",
            "-p:WasmBuildNative=true",
        ],
    )
    .with_context(|| "dotnet publish")?;

    let bin_dir = cs_dir().join("bin").join("Release");
    let bundle = bin_dir
        .join("net8.0")
        .join("browser-wasm")
        .join("AppBundle")
        .join("_framework");
    let fallback = bin_dir.join("net8.0").join("browser-wasm").join("publish");
    let publish_dir = if bundle.is_dir() { bundle } else { fallback };
    if !publish_dir.is_dir() {
        anyhow::bail!("publish output not found: {}", publish_dir.display());
    }

    let dst = paths::public_wasm_dir().join("csharp");
    if dst.is_dir() {
        std::fs::remove_dir_all(&dst).with_context(|| format!("rm -rf {}", dst.display()))?;
    }
    copy_tree(&publish_dir, &dst)?;

    let bc_js = paths::adapters_dir()
        .join("csharp")
        .join("Saikuro")
        .join("src")
        .join("BroadcastChannel")
        .join("wwwroot")
        .join("Saikuro.BroadcastChannel.js");
    if bc_js.is_file() {
        std::fs::copy(&bc_js, dst.join("Saikuro.BroadcastChannel.js"))
            .with_context(|| format!("copy {}", bc_js.display()))?;
    }
    Ok(())
}

pub(super) fn demo_wasm_python() -> anyhow::Result<()> {
    let dist = paths::public_wasm_dir().join("python");
    std::fs::create_dir_all(&dist)?;

    run::run(
        &root(),
        "python3",
        [
            "-m",
            "pip",
            "wheel",
            "--no-deps",
            "--no-binary",
            "msgpack",
            "--wheel-dir",
            dist.display().to_string().as_str(),
            "msgpack==1.2.1",
        ],
    )
    .with_context(|| "msgpack pure-python wheel")?;

    run::run(
        &paths::adapters_dir().join("python"),
        "uv",
        [
            "build",
            "--wheel",
            "--out-dir",
            dist.display().to_string().as_str(),
        ],
    )
    .with_context(|| "saikuro python wheel")?;

    std::fs::copy(
        wasm_dir().join("python").join("insight.py"),
        dist.join("insight.py"),
    )
    .with_context(|| "copy insight.py")?;
    Ok(())
}

/// Full demo build: every WASM provider plus the Vite bundle.
pub(super) fn demo_build() -> anyhow::Result<()> {
    ensure_public_dirs()?;
    for (label, step) in [
        (
            "rust wasm",
            demo_wasm_rust_all as fn() -> anyhow::Result<()>,
        ),
        ("c wasm", demo_wasm_c),
        ("cpp wasm", demo_wasm_cpp),
        ("csharp wasm", demo_wasm_csharp),
        ("python files", demo_wasm_python),
    ] {
        println!("building {label}...");
        step()?;
    }
    run::run(&demo_dir(), "npm", ["run", "build"]).with_context(|| "npm run build (demo)")
}

/// Start the Vite dev server with live WASM rebuilds.
pub(super) fn demo_dev() -> anyhow::Result<()> {
    run::run(&demo_dir(), "node", ["dev.mjs"])
}

fn copy_tree(from: &Path, to: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from).with_context(|| format!("read {}", from.display()))? {
        let entry = entry?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_tree(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst).with_context(|| format!("copy {}", src.display()))?;
        }
    }
    Ok(())
}
