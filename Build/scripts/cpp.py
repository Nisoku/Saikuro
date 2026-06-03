"""C++ adapter commands."""

import sys
from pathlib import Path

from shared.constants import CPP_DIR
from shared.run import run
from shared.format import check_clang, ensure_clang_format

CMDS = {
    "test": ["sh", "-c", "cmake --build build && ctest --test-dir build --output-on-failure"],
}

_FMT_DIRS = [CPP_DIR / "src", CPP_DIR / "include", CPP_DIR / "tests"]


def _ensure_cmake() -> None:
    import shutil, subprocess
    if shutil.which("cmake"):
        return
    print("cmake not found; installing...", flush=True)
    if sys.platform.startswith('linux'):
            subprocess.run(["sudo", "apt-get", "update"], check=True)
            subprocess.run(["sudo", "apt-get", "install", "-y", "cmake"], check=True)


def _ensure_emsdk() -> None:
    import shutil, subprocess
    if shutil.which("emcc"):
        return
    print("emcc not found; installing Emscripten SDK (emsdk)...", flush=True)
    emsdk_dir = Path.home() / ".emsdk"
    if not emsdk_dir.exists():
        subprocess.run(["git", "clone", "https://github.com/emscripten-core/emsdk.git", str(emsdk_dir)], check=True)
    subprocess.run([str(emsdk_dir / "emsdk"), "install", "latest"], check=True, cwd=str(emsdk_dir))
    subprocess.run([str(emsdk_dir / "emsdk"), "activate", "latest"], check=True, cwd=str(emsdk_dir))
    print(f"Emscripten SDK installed to {emsdk_dir}.", flush=True)
    print(f"To enable `emcc` in your shell, add the following to your shell rc:\n\n    source {emsdk_dir}/emsdk_env.sh\n", flush=True)


def setup() -> int:
    ensure_clang_format()
    _ensure_cmake()
    _ensure_emsdk()
    return run(["cmake", "-S", ".", "-B", "build"], cwd=CPP_DIR)


def fmt_check() -> int:
    return check_clang(_FMT_DIRS, ["*.[ch]pp", "*.h"], cwd=CPP_DIR)


def main() -> None:
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"
    if cmd == "fmt_check":
        sys.exit(fmt_check())
    if cmd == "setup":
        sys.exit(setup())
    if cmd == "check":
        failed = any([fmt_check() != 0, setup() != 0, run(CMDS["test"], cwd=CPP_DIR) != 0])
        sys.exit(1 if failed else 0)
    if cmd in CMDS:
        sys.exit(run(CMDS[cmd], cwd=CPP_DIR))
    print(f"Usage: {sys.argv[0]} <check|fmt_check|setup|{'|'.join(CMDS)}>")
    sys.exit(1)


if __name__ == "__main__":
    main()
