"""Rust workspace + adapter commands."""

import subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "adapters" / "rust"

CMDS = {
    "setup": ["rustup", "target", "add", "wasm32-unknown-unknown"],
    "wasm_check": ["cargo", "check", "--target", "wasm32-unknown-unknown", "-p", "saikuro-tests"],
    "test": ["cargo", "test", "--workspace"],
    "adapter_test": ["cargo", "test", "-p", "saikuro"],
}


def run(cmd: list[str], cwd: Path = ROOT) -> int:
    return subprocess.run(cmd, cwd=cwd).returncode


def _try_fmt(extra_args: list[str], cwd: Path, label: str) -> None:
    cmd = ["cargo", "fmt", *extra_args, "--", "--check"]
    result = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
    if result.returncode == 0:
        return
    print(result.stdout, result.stderr, sep="", end="", flush=True)
    subprocess.run(["cargo", "fmt", *extra_args], cwd=cwd)
    print(f"[WARN] {label} format issues auto-fixed. Stage changes before committing.", flush=True)


def fmt_check() -> int:
    _try_fmt(["--all"], ROOT, "Rust workspace")
    _try_fmt([], ADAPTER, "Rust adapter")
    return 0


def lint() -> int:
    return run(["cargo", "clippy", "--workspace", "--", "-D", "warnings"]) + \
           run(["cargo", "clippy", "--", "-D", "warnings"], cwd=ADAPTER)


def main() -> None:
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"

    if cmd == "fmt_check":
        fmt_check()
        return
    if cmd == "lint":
        exit(lint())
    if cmd == "check":
        exit(sum([fmt_check(), lint(), run(CMDS["test"]),
                  run(CMDS["wasm_check"]), run(CMDS["adapter_test"])]))
    if cmd in CMDS:
        exit(run(CMDS[cmd]))
    print(f"Usage: {sys.argv[0]} <check|fmt_check|lint|{'|'.join(CMDS)}>")
    exit(1)


if __name__ == "__main__":
    main()
