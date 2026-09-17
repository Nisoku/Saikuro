use std::path::{Path, PathBuf};

pub fn repo_root() -> PathBuf {
    let manifest =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is always set by cargo");
    Path::new(&manifest)
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root must exist")
}

pub fn build_dir() -> PathBuf {
    repo_root().join("Build")
}

pub fn tests_dir() -> PathBuf {
    repo_root().join("Tests")
}

pub fn adapters_dir() -> PathBuf {
    build_dir().join("adapters")
}

pub fn demo_dir() -> PathBuf {
    repo_root().join("Demo")
}

pub fn demo_wasm_dir() -> PathBuf {
    demo_dir().join("wasm")
}

pub fn public_wasm_dir() -> PathBuf {
    demo_dir().join("public").join("wasm")
}

pub fn tools_dir() -> PathBuf {
    repo_root().join("tools")
}

pub fn baselines_dir() -> PathBuf {
    tools_dir().join("baselines")
}

pub fn tools_manifest() -> PathBuf {
    tools_dir().join("tools.toml")
}

pub fn engine_manifest() -> PathBuf {
    repo_root().join("Cargo.toml")
}

pub fn deny_config() -> PathBuf {
    build_dir().join("deny.toml")
}
