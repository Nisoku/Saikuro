import os
import shutil
import subprocess
import sys
from pathlib import Path

StrPath = str | Path


def ensure_clang_format() -> None:
    if shutil.which("clang-format"):
        return
    print("clang-format not found; installing...", flush=True)
    if sys.platform == "darwin":
        subprocess.run(["brew", "install", "clang-format"], check=True)
    elif sys.platform.startswith("linux"):
        subprocess.run(["sudo", "apt-get", "install", "-y", "clang-format"], check=True)
    else:
        pip = shutil.which("pip3") or shutil.which("pip")
        if pip:
            subprocess.run([pip, "install", "clang-format"], check=True)


def _resolve_cwd(cwd: StrPath | None) -> str | None:
    return str(cwd) if isinstance(cwd, Path) else cwd


def check(label: str, check_cmd: list[str], fix_cmd: list[str], cwd: StrPath | None = None) -> int:
    cwd_str = _resolve_cwd(cwd)
    result = subprocess.run(check_cmd, cwd=cwd_str, capture_output=True, text=True)
    if result.returncode == 0:
        return 0
    print(result.stdout, result.stderr, sep="", end="", flush=True)
    if os.environ.get("CI"):
        return result.returncode
    subprocess.run(fix_cmd, cwd=cwd_str)
    print(f"[WARN] {label} format issues auto-fixed. Stage changes before committing.", flush=True)
    return result.returncode


def check_clang(dirs: list[Path], patterns: list[str], cwd: StrPath | None = None) -> int:
    clang = shutil.which("clang-format")
    if clang is None:
        print("[WARN] clang-format not found; skipping C/C++ format check", flush=True)
        return 0
    sources: list[Path] = []
    for d in dirs:
        if d.is_dir():
            for pat in patterns:
                sources.extend(d.rglob(pat))
    if not sources:
        return 0
    cwd_str = _resolve_cwd(cwd)
    cmd = [clang, "--dry-run", "-Werror"] + [str(s) for s in sources]
    result = subprocess.run(cmd, cwd=cwd_str, capture_output=True, text=True)
    if result.returncode == 0:
        return 0
    print(result.stdout, result.stderr, sep="", end="", flush=True)
    if os.environ.get("CI"):
        return result.returncode
    subprocess.run([clang, "-i"] + [str(s) for s in sources], cwd=cwd_str)
    print("[WARN] C/C++ format issues auto-fixed. Stage changes before committing.", flush=True)
    return result.returncode
