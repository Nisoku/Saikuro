use std::io::Write;
use std::path::PathBuf;

use anyhow::Context;

use crate::paths;
use crate::run;

mod c;
mod cpp;
mod csharp;
mod python;
mod rust;
mod typescript;

use c::*;
use cpp::*;
use csharp::*;
use python::*;
use rust::*;
use typescript::*;

fn root() -> PathBuf {
    paths::repo_root()
}

fn ci() -> bool {
    std::env::var("CI").is_ok()
}

/// Verify a formatter/linter, auto-fixing locally before reporting failure.
fn run_fix_step(
    label: &str,
    cwd: &std::path::Path,
    program: &str,
    check_args: &[&str],
    fix_args: &[&str],
) -> anyhow::Result<()> {
    let out =
        run::run_capture(cwd, program, check_args).with_context(|| format!("{label} check"))?;
    if out.status.success() {
        return Ok(());
    }
    std::io::stdout().write_all(&out.stdout)?;
    std::io::stderr().write_all(&out.stderr)?;
    if !ci() {
        run::run(cwd, program, fix_args).with_context(|| format!("{label} auto-fix"))?;
        println!("[WARN] {label} issues auto-fixed. Stage changes before committing.");
    }
    anyhow::bail!("{label} failed")
}

/// Formatting verb: run the fixer, never the check.
fn run_formatter(
    label: &str,
    cwd: &std::path::Path,
    program: &str,
    fix_args: &[&str],
) -> anyhow::Result<()> {
    run::run(cwd, program, fix_args).with_context(|| format!("{label} format"))
}

fn collect_recursive(dir: &std::path::Path, patterns: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(p) = stack.pop() {
        if let Ok(read) = std::fs::read_dir(&p) {
            for entry in read.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    if patterns.iter().any(|pat| glob_match(pat, name)) {
                        out.push(path);
                    }
                }
            }
        }
    }
    out.sort();
    out
}

/// Minimal `*`-and-`?` glob for the small fixed pattern set above.
fn glob_match(pattern: &str, name: &str) -> bool {
    fn m(pat: &[char], name: &[char]) -> bool {
        match (pat.first(), name.first()) {
            (None, None) => true,
            (Some('*'), _) => m(&pat[1..], name) || (!name.is_empty() && m(pat, &name[1..])),
            (Some(p), Some(n)) => (*p == *n || *p == '?') && m(&pat[1..], &name[1..]),
            _ => false,
        }
    }
    m(
        &pattern.chars().collect::<Vec<_>>(),
        &name.chars().collect::<Vec<_>>(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Lang {
    Rust,
    Python,
    TypeScript,
    CSharp,
    C,
    Cpp,
}

impl Lang {
    pub fn name(self) -> &'static str {
        match self {
            Lang::Rust => "rust",
            Lang::Python => "python",
            Lang::TypeScript => "typescript",
            Lang::CSharp => "csharp",
            Lang::C => "c",
            Lang::Cpp => "cpp",
        }
    }
}

pub const ALL_LANGS: [Lang; 6] = [
    Lang::Rust,
    Lang::Python,
    Lang::TypeScript,
    Lang::CSharp,
    Lang::C,
    Lang::Cpp,
];

/// Aggregate `just check` equivalent.
pub fn check_all() -> anyhow::Result<()> {
    for lang in ALL_LANGS {
        run_lang(lang, "check")?;
    }
    Ok(())
}

pub fn format_all() -> anyhow::Result<()> {
    for lang in ALL_LANGS {
        run_lang(lang, "format")?;
    }
    Ok(())
}

pub fn lint_all() -> anyhow::Result<()> {
    rust_lint()?;
    python_lint()?;
    typescript_lint()?;
    Ok(())
}

pub fn clean_all() -> anyhow::Result<()> {
    run::cargo(&root(), &["clean"])?;
    python_clean()?;
    typescript_clean()?;
    csharp_clean()?;
    cpp_clean()?;
    c_format()?;
    super::qemu::clean()?;
    for path in [
        paths::demo_dir().join("public").join("wasm"),
        paths::demo_dir().join("node_modules"),
        paths::demo_dir().join("dist"),
    ] {
        if path.is_dir() {
            std::fs::remove_dir_all(&path).with_context(|| format!("rm {}", path.display()))?;
        }
    }
    Ok(())
}

pub fn run_lang(lang: Lang, verb: &str) -> anyhow::Result<()> {
    match (lang, verb) {
        (Lang::Rust, "check") => rust_check(),
        (Lang::Rust, "format") => rust_format(),
        (Lang::Rust, "lint") => rust_lint(),
        (Lang::Rust, "setup") => run::run(
            &root(),
            "rustup",
            ["target", "add", "wasm32-unknown-unknown"],
        ),
        (Lang::Rust, "test") => run::cargo(&root(), &["test", "--workspace"]),
        (Lang::Rust, "clean") => run::cargo(&root(), &["clean"]),
        (Lang::Rust, "wasm_check") => run::cargo(
            &root(),
            &rust_wasm_clippy_args()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        ),
        (Lang::Rust, "adapter_test") => run::cargo(&root(), &["test", "-p", "saikuro"]),

        (Lang::Python, "check") => python_check(),
        (Lang::Python, "format") => python_format(),
        (Lang::Python, "lint") => python_lint(),
        (Lang::Python, "setup") => python_setup(),
        (Lang::Python, "test") => python_test(),
        (Lang::Python, "clean") => python_clean(),

        (Lang::TypeScript, "check") => typescript_check(),
        (Lang::TypeScript, "format") => typescript_format(),
        (Lang::TypeScript, "lint") => typescript_lint(),
        (Lang::TypeScript, "setup") => run::run(&ts_dir(), "npm", ["install"]),
        (Lang::TypeScript, "test") => run::run(&ts_dir(), "npm", ["test"]),
        (Lang::TypeScript, "clean") => typescript_clean(),

        (Lang::CSharp, "check") => csharp_check(),
        (Lang::CSharp, "format") => csharp_format(),
        (Lang::CSharp, "setup") => {
            let project = cs_src().display().to_string();
            run::run(&cs_dir(), "dotnet", ["restore", project.as_str()])
        }
        (Lang::CSharp, "test") => {
            let project = cs_test().display().to_string();
            run::run(
                &cs_dir(),
                "dotnet",
                ["test", project.as_str(), "-c", "Release"],
            )
        }
        (Lang::CSharp, "clean") => csharp_clean(),

        (Lang::C, "check") => c_check(),
        (Lang::C, "format") => c_format(),
        (Lang::C, "build") => run::cargo(&root(), &["build", "-p", "saikuro-c"]),
        (Lang::C, "test") => run::cargo(&root(), &["test", "-p", "saikuro-c"]),
        (Lang::C, "clean") => run::cargo(&root(), &["clean", "-p", "saikuro-c"]),

        (Lang::Cpp, "check") => cpp_check(),
        (Lang::Cpp, "format") => cpp_format(),
        (Lang::Cpp, "setup") => cpp_setup(),
        (Lang::Cpp, "test") => cpp_test(),
        (Lang::Cpp, "clean") => cpp_clean(),

        (lang, verb) => {
            anyhow::bail!("no such verb '{}' for language '{}'", verb, lang.name())
        }
    }
}

fn python_check() -> anyhow::Result<()> {
    python_format_check()?;
    python_lint()?;
    python_test()
}
