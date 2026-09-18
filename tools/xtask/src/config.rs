use std::fs;
use std::path::Path;

use anyhow::Context;
use serde::Deserialize;

/// How a tool is installed.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// `cargo binstall <pkg>@<version>` (cause speeeed).
    Binstall,
    /// Expect a binary from the host package manager, only verified.
    System,
}

impl Source {
    pub fn describe(self) -> &'static str {
        match self {
            Source::Binstall => "cargo binstall",
            Source::System => "host package",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Rust {
    pub toolchain: String,
    pub nightly_channel: String,
}

#[derive(Debug, Deserialize)]
pub struct Tool {
    pub name: String,
    #[serde(default)]
    pub pkg: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub bin: String,
    pub source: Source,
    #[serde(default)]
    pub also: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub rust: Rust,
    pub tools: Vec<Tool>,
}

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("read tool manifest {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parse tool manifest {}", path.display()))
    }
}
