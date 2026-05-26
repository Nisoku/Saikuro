"""C++ adapter commands."""

import os, shutil, subprocess, sys
from pathlib import Path

DIR = Path(__file__).resolve().parents[1] / "adapters" / "cpp"

CMDS = {
    "test": ["sh", "-c", "cmake --build build && ctest --test-dir build --output-on-failure"],
    "check": [],
}

# Source directories to format.
_FMT_DIRS = [DIR / "src", DIR / "include", DIR / "tests"]


def run(cmd: list[str]) -> int:
    return subprocess.run(cmd, cwd=DIR).returncode


def _ensure_clang_format() -> None:
    if shutil.which("clang-format"):
        return
    print("clang-format not found; installing...", flush=True)
    platform = sys.platform
    if platform == "darwin":
        subprocess.run(["brew", "install", "clang-format"], check=True)
    elif platform.startswith("linux"):
        subprocess.run(["sudo", "apt-get", "install", "-y", "clang-format"], check=True)
    else:
        pip = shutil.which("pip3") or shutil.which("pip")
        if pip:
            subprocess.run([pip, "install", "clang-format"], check=True)


def _ensure_cmake() -> None:
    if shutil.which("cmake"):
        return
    print("cmake not found; installing...", flush=True)
    subprocess.run(["sudo", "apt-get", "install", "-y", "cmake"], check=True)


def setup() -> int:
    _ensure_clang_format()
    _ensure_cmake()
    return run(["cmake", "-S", ".", "-B", "build"])


def fmt_check() -> int:
    clang = shutil.which("clang-format")
    if clang is None:
        print("[WARN] clang-format not found; skipping C++ format check", flush=True)
        return 0
    sources = []
    for d in _FMT_DIRS:
        if d.is_dir():
            sources.extend(d.rglob("*.[ch]pp"))
            sources.extend(d.rglob("*.h"))
    if not sources:
        return 0
    cmd = [clang, "--dry-run", "-Werror"] + [str(s) for s in sources]
    result = subprocess.run(cmd, cwd=DIR, capture_output=True, text=True)
    if result.returncode == 0:
        return 0
    print(result.stdout, result.stderr, sep="", end="", flush=True)
    if os.environ.get("CI"):
        return result.returncode
    subprocess.run([clang, "-i"] + [str(s) for s in sources], cwd=DIR)
    print("[WARN] C++ format issues auto-fixed. Stage changes before committing.", flush=True)
    return result.returncode


def main() -> None:
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"
    if cmd == "fmt_check":
        exit(fmt_check())
    elif cmd == "setup":
        exit(setup())
    elif cmd == "check":
        failed = any([fmt_check() != 0, setup() != 0, run(CMDS["test"]) != 0])
        exit(1 if failed else 0)
    if cmd in CMDS:
        exit(run(CMDS[cmd]))
    print(f"Usage: {sys.argv[0]} <check|fmt_check|setup|{'|'.join(CMDS)}>")
    exit(1)


if __name__ == "__main__":
    main()
