//! Saikuro build system

mod config;
mod gates;
mod languages;
mod matrix;
mod paths;
mod qemu;
mod run;
mod setup;

use anyhow::Context;
use clap::{Args, Parser, Subcommand};

const ABOUT: &str = "Saikuro multi-target development orchestrator";

#[derive(Parser)]
#[command(name = "xtask", about = ABOUT, version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// format, lint, build and test every language.
    Check,
    /// Run workspace tests on a family: native (default), wasm, embedded, wasi.
    Test {
        #[arg(value_enum, default_value_t = TestTarget::Native)]
        target: TestTarget,
    },
    /// Cross-compile check across the full engine x target matrix.
    Matrix(MatrixArgs),
    /// QEMU embedded build/run.
    Qemu {
        #[arg(value_enum, default_value_t = QemuVerb::Check)]
        verb: QemuVerb,
    },
    /// Run a verb for one language adapter (including the web demo).
    Lang {
        #[arg(value_enum)]
        lang: languages::Lang,
        #[arg(value_enum)]
        verb: LangVerb,
    },
    /// Lint gates for rust, python, typescript.
    Lint,
    /// Run formatters for every language.
    Format,
    /// Purge build artifacts for every language.
    Clean,
    /// Verify/install the pinned toolchain (tools/tools.toml).
    Setup {
        /// Fail if any tool is missing instead of installing.
        #[arg(long)]
        check: bool,
    },
    /// Print the pinned toolchain table.
    Tools,
    /// cargo-deny check against Build/deny.toml.
    Deny,
    /// cargo-audit dependency advisories.
    Audit,
    /// Miri over the embedded-engine suite.
    Miri,
    /// typos source spell check.
    Typos,
    /// geiger unsafe baseline snapshot gate.
    Geiger {
        /// Rewrite tools/baselines/geiger.baseline instead of checking.
        #[arg(long)]
        update: bool,
    },
    // TODO: Disabled until kbknapp/cargo-outdated#122 merges
    // /// cargo-outdated with a failure on any drift.
    // Outdated,
    /// cargo-spellcheck doc spell analysis.
    Spellcheck,
    /// rustdoc intra-doc link gate.
    Deadlinks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum TestTarget {
    Native,
    Wasm,
    Embedded,
    Wasi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum QemuVerb {
    Setup,
    BuildArm,
    BuildRiscv,
    Build,
    Run,
    Check,
    RunArm,
    RunRiscv,
    Test,
    Clean,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum LangVerb {
    Check,
    Format,
    Lint,
    Setup,
    Test,
    Clean,
    Build,
    WasmCheck,
    AdapterTest,
    Dev,
    BuildC,
    BuildCpp,
    BuildCsharp,
    BuildRustRuntime,
    BuildRustProvider,
    BuildRust,
    BuildPython,
}

impl LangVerb {
    fn as_str(self) -> &'static str {
        match self {
            LangVerb::Check => "check",
            LangVerb::Format => "format",
            LangVerb::Lint => "lint",
            LangVerb::Setup => "setup",
            LangVerb::Test => "test",
            LangVerb::Clean => "clean",
            LangVerb::Build => "build",
            LangVerb::WasmCheck => "wasm_check",
            LangVerb::AdapterTest => "adapter_test",
            LangVerb::Dev => "dev",
            LangVerb::BuildC => "build-c",
            LangVerb::BuildCpp => "build-cpp",
            LangVerb::BuildCsharp => "build-csharp",
            LangVerb::BuildRustRuntime => "build-rust-runtime",
            LangVerb::BuildRustProvider => "build-rust-provider",
            LangVerb::BuildRust => "build-rust",
            LangVerb::BuildPython => "build-python",
        }
    }
}

#[derive(Args)]
struct MatrixArgs {
    /// Crate to check (default saikuro-runtime).
    #[arg(long)]
    crate_name: Option<String>,
    /// Check every workspace member.
    #[arg(long, conflicts_with = "crate_name")]
    all_crates: bool,
    /// Write results as JSON to this path.
    #[arg(long)]
    json: Option<std::path::PathBuf>,
    /// Print full output for failing combos.
    #[arg(long)]
    verbose: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cli.command.run()
}

impl Command {
    fn run(self) -> anyhow::Result<()> {
        let root = paths::repo_root();
        match self {
            Command::Check => languages::check_all(),
            Command::Test { target } => match target {
                TestTarget::Native => run::run(
                    &root,
                    "cargo",
                    [
                        "run",
                        "-q",
                        "-p",
                        "saikuro-tests",
                        "--bin",
                        "native",
                        "--features",
                        "native",
                    ],
                )
                .context("native runner tests"),
                TestTarget::Wasm => wasm_tests(),
                TestTarget::Embedded => qemu::test_embedded(),
                TestTarget::Wasi => wasi_tests(),
            },
            Command::Matrix(args) => matrix::run_matrix(
                &root,
                args.all_crates,
                args.crate_name.as_deref(),
                args.json.as_deref(),
            ),
            Command::Qemu { verb } => match verb {
                QemuVerb::Setup => qemu::setup(),
                QemuVerb::BuildArm => qemu::build_arm(),
                QemuVerb::BuildRiscv => qemu::build_riscv(),
                QemuVerb::Build => qemu::build_all(),
                QemuVerb::Run => qemu::test_embedded(),
                QemuVerb::Test => qemu::test_embedded(),
                QemuVerb::Check => qemu::check(),
                QemuVerb::RunArm => qemu::run_arm(),
                QemuVerb::RunRiscv => qemu::run_riscv(),
                QemuVerb::Clean => qemu::clean(),
            },
            Command::Lang { lang, verb } => languages::run_lang(lang, verb.as_str()),
            Command::Lint => languages::lint_all(),
            Command::Format => languages::format_all(),
            Command::Clean => languages::clean_all(),
            Command::Setup { check } => setup::sync(check),
            Command::Tools => setup::list(),
            Command::Deny => gates::deny(),
            Command::Audit => gates::audit(),
            Command::Miri => gates::miri(),
            Command::Typos => gates::typos(),
            Command::Geiger { update } => gates::geiger(update),
            // Command::Outdated => gates::outdated(),
            Command::Spellcheck => gates::spellcheck(),
            Command::Deadlinks => gates::deadlinks(),
        }
    }
}

/// Run the saikuro-tests wasm suite through wasm-bindgen-test-runner.
fn wasm_tests() -> anyhow::Result<()> {
    run::run(
        &paths::repo_root(),
        "cargo",
        [
            "test",
            "-p",
            "saikuro-tests",
            "--target",
            "wasm32-unknown-unknown",
            "--no-default-features",
            "--features",
            "wasm",
        ],
    )
    .context("wasm tests")
}

/// Run the wasi suites by executing each runner bin under wasmtime
fn wasi_tests() -> anyhow::Result<()> {
    for (features, target, bin, label) in [
        (
            "wasi-preview1",
            "wasm32-wasip1",
            "wasi-preview1",
            "wasi-preview1",
        ),
        (
            "wasi-preview2",
            "wasm32-wasip2",
            "wasi-preview2",
            "wasi-preview2",
        ),
    ] {
        run::run(
            &paths::repo_root(),
            "cargo",
            [
                "run",
                "-q",
                "-p",
                "saikuro-tests",
                "--target",
                target,
                "--no-default-features",
                "--features",
                features,
                "--bin",
                bin,
            ],
        )
        .with_context(|| format!("{label} tests"))?;
    }
    Ok(())
}
