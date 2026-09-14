use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Output};

/// Run a program to completion, forwarding its stdio. Fails with the exit
/// status on a non-zero return.
pub fn run<I, S>(cwd: &Path, program: &str, args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let status = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .status()
        .map_err(|e| anyhow::anyhow!("failed to spawn {program}: {e}"))?;
    if !status.success() {
        anyhow::bail!("{program} exited with {status}");
    }
    Ok(())
}

/// Run a program, capturing stdout+stderr. Never throws on a non-zero exit.
pub fn run_capture<I, S>(cwd: &Path, program: &str, args: I) -> anyhow::Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| anyhow::anyhow!("failed to spawn {program}: {e}"))
}

pub fn cargo(cwd: &Path, args: &[&str]) -> anyhow::Result<()> {
    run(cwd, "cargo", args)
}

/// True when the program is on PATH.
pub fn which(program: &str) -> bool {
    fn probe(program: &str, arg: &str) -> bool {
        Command::new(program)
            .arg(arg)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    probe(program, "--version")
        || probe(program, "--help")
        || Command::new("sh")
            .arg("-c")
            .arg(format!("command -v {program}"))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
}
