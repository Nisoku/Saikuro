use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::paths;
use crate::run;

const BINARY: &str = "wasm-opt";
const SHA_MARKER: &str = "source.sha";
const OSX_DEPLOYMENT_TARGET: &str = "11.0";

fn pinned_sha(source: &Path) -> anyhow::Result<String> {
    let out = run::run_capture_ok(source, "git", ["rev-parse", "HEAD"])
        .with_context(|| "git rev-parse HEAD in the vendored binaryen submodule")?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn cached_sha(cache: &Path) -> Option<String> {
    fs::read_to_string(cache.join(SHA_MARKER))
        .ok()
        .map(|s| s.trim().to_string())
}

fn short(sha: &str) -> &str {
    &sha[..7.min(sha.len())]
}

/// Build the vendored, asyncify-capable `wasm-opt` and return its path.
pub fn ensure() -> anyhow::Result<PathBuf> {
    let source = paths::binaryen_dir();
    if !source.join("CMakeLists.txt").is_file() {
        anyhow::bail!(
            "vendored binaryen submodule is missing; run `git submodule update --init --recursive` ({})",
            source.display()
        );
    }
    let cache = paths::wasmopt_cache_dir();
    let build = cache.join("build");
    let built = build.join("bin").join(BINARY);
    let sha = pinned_sha(&source)?;

    if built.is_file() && cached_sha(&cache).as_deref() == Some(sha.as_str()) {
        println!("wasm-opt  cached (binaryen {})", short(&sha));
        return Ok(built);
    }

    println!("building wasm-opt from binaryen {} ...", short(&sha));
    let root = paths::repo_root();
    let source_arg = source.to_string_lossy().into_owned();
    let build_arg = build.to_string_lossy().into_owned();
    let jobs = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    let osx_arg = format!("-DCMAKE_OSX_DEPLOYMENT_TARGET={OSX_DEPLOYMENT_TARGET}");
    // Configure only when the build dir is fresh.
    if !build.join("CMakeCache.txt").is_file() {
        let mut configure: Vec<&str> = vec![
            "-S",
            source_arg.as_str(),
            "-B",
            build_arg.as_str(),
            "-DCMAKE_BUILD_TYPE=Release",
            "-DBUILD_TESTS=OFF",
        ];
        if cfg!(target_os = "macos") {
            configure.push(&osx_arg);
        }
        if run::which("ninja") {
            configure.push("-G");
            configure.push("Ninja");
        }
        run::run(&root, "cmake", configure).with_context(|| "cmake configure binaryen")?;
    }
    run::run(
        &root,
        "cmake",
        [
            "--build",
            build_arg.as_str(),
            "--target",
            BINARY,
            "--parallel",
            &jobs.to_string(),
        ],
    )
    .with_context(|| "cmake build wasm-opt")?;

    if !built.is_file() {
        anyhow::bail!("cmake did not produce {}", built.display());
    }
    fs::create_dir_all(&cache).with_context(|| format!("create {}", cache.display()))?;
    fs::write(cache.join(SHA_MARKER), format!("{sha}\n"))
        .with_context(|| "write source.sha marker")?;
    println!("wasm-opt  built (binaryen {})", short(&sha));
    Ok(built)
}
