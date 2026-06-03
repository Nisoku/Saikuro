"""C adapter commands."""

import os, shutil, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DIR = ROOT / "adapters" / "c"

CMDS = {
    "build": ["cargo", "build", "-p", "saikuro-c"],
    "test": ["cargo", "test", "-p", "saikuro-c"],
    "check": [],
}

_FMT_DIRS = [DIR / "include"]


def run(cmd: list[str]) -> int:
    return subprocess.run(cmd, cwd=ROOT).returncode


def _ensure_clang_format() -> None:
    if shutil.which("clang-format"):
        return
    print("clang-format not found; installing...", flush=True)
    platform = sys.platform
    if platform == "darwin":
        subprocess.run(["brew", "install", "clang-format"], check=True)
    elif platform.startswith("linux"):
        subprocess.run(["sudo", "apt-get", "install", "-y", "clang-format"], check=True)


def fmt_check() -> int:
    rc = 0

    clang = shutil.which("clang-format")
    if clang is None:
        print("[WARN] clang-format not found; skipping C header format check", flush=True)
    else:
        sources = []
        for d in _FMT_DIRS:
            if d.is_dir():
                sources.extend(d.rglob("*.h"))
        if sources:
            cmd = [clang, "--dry-run", "-Werror"] + [str(s) for s in sources]
            result = subprocess.run(cmd, cwd=DIR, capture_output=True, text=True)
            if result.returncode != 0:
                print(result.stdout, result.stderr, sep="", end="", flush=True)
                if os.environ.get("CI"):
                    rc += result.returncode
                else:
                    subprocess.run([clang, "-i"] + [str(s) for s in sources], cwd=DIR)
                    print("[WARN] C header format issues auto-fixed. Stage changes before committing.", flush=True)
                    rc += result.returncode

    result = subprocess.run(
        ["cargo", "fmt", "-p", "saikuro-c", "--", "--check"],
        cwd=ROOT, capture_output=True, text=True,
    )
    if result.returncode != 0:
        print(result.stdout, result.stderr, sep="", end="", flush=True)
        if os.environ.get("CI"):
            rc += result.returncode
        else:
            subprocess.run(["cargo", "fmt", "-p", "saikuro-c"], cwd=ROOT)
            print("[WARN] C crate Rust format issues auto-fixed. Stage changes before committing.", flush=True)
            rc += result.returncode

    return rc


def main() -> None:
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"
    if cmd == "fmt_check":
        exit(fmt_check())
    elif cmd == "check":
        failed = any([fmt_check() != 0, run(CMDS["build"]) != 0, run(CMDS["test"]) != 0])
        exit(1 if failed else 0)
    elif cmd in CMDS:
        exit(run(CMDS[cmd]))
    print(f"Usage: {sys.argv[0]} <check|fmt_check|{'|'.join(CMDS)}>")
    exit(1)


if __name__ == "__main__":
    main()
