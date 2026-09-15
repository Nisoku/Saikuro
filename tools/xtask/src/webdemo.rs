use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::paths;
use crate::run;

fn root() -> PathBuf {
    paths::repo_root()
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

pub fn wasm_runtime() -> anyhow::Result<()> {
    wasm_pack(&paths::demo_wasm_dir().join("runtime"), "runtime")
}

pub fn wasm_rust() -> anyhow::Result<()> {
    wasm_pack(&paths::demo_wasm_dir().join("rust"), "rust")
}

pub fn wasm_rust_all() -> anyhow::Result<()> {
    wasm_runtime()?;
    wasm_rust()
}

pub fn wasm_c() -> anyhow::Result<()> {
    wasm_pack(&paths::demo_wasm_dir().join("c"), "c")
}

pub fn wasm_cpp() -> anyhow::Result<()> {
    wasm_pack(&paths::demo_wasm_dir().join("cpp"), "cpp")
}

const CSHARP_PROJECT: &str = "InsightLab";

pub fn wasm_csharp() -> anyhow::Result<()> {
    let project = paths::demo_wasm_dir()
        .join("csharp")
        .join(CSHARP_PROJECT)
        .join("InsightLab.csproj");
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

    let bin_dir = paths::demo_wasm_dir()
        .join("csharp")
        .join(CSHARP_PROJECT)
        .join("bin")
        .join("Release");
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

pub fn wasm_python() -> anyhow::Result<()> {
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

    let stale = dist.join("saikuro-0.1.0-py3-none-any.whl");
    if stale.is_file() {
        std::fs::remove_file(&stale).with_context(|| format!("remove {}", stale.display()))?;
    }

    std::fs::copy(
        paths::demo_wasm_dir().join("python").join("insight.py"),
        dist.join("insight.py"),
    )
    .with_context(|| "copy insight.py")?;
    Ok(())
}

pub fn setup_dependencies() -> anyhow::Result<()> {
    for (dir, label) in [
        (
            paths::adapters_dir().join("typescript"),
            "typescript adapter",
        ),
        (paths::demo_dir(), "demo"),
    ] {
        if !dir.is_dir() {
            anyhow::bail!("{label} dir missing: {}", dir.display());
        }
        run::run(&dir, "npm", ["install"]).with_context(|| format!("npm install ({label})"))?;
    }
    Ok(())
}

pub fn build_all() -> anyhow::Result<()> {
    ensure_public_dirs()?;
    for (label, step) in [
        ("rust wasm", wasm_rust_all as fn() -> anyhow::Result<()>),
        ("c wasm", wasm_c),
        ("cpp wasm", wasm_cpp),
        ("csharp wasm", wasm_csharp),
        ("python files", wasm_python),
    ] {
        println!("building {label}...");
        step()?;
    }
    run::run(&paths::demo_dir(), "npm", ["run", "build"]).with_context(|| "npm run build (demo)")
}

pub fn dev_server() -> anyhow::Result<()> {
    run::run(&paths::demo_dir(), "node", ["dev.mjs"])
}

pub fn typecheck() -> anyhow::Result<()> {
    run::run(&paths::demo_dir(), "npm", ["run", "typecheck"])
        .with_context(|| "npm run typecheck (demo)")
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
