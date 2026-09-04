"""Python adapter commands."""

import argparse
import os
import subprocess
import sys

from shared.constants import PYTHON_DIR
from shared.run import run
from shared.format import check

CMDS = {
    "setup": ["uv", "sync", "--extra", "dev", "--extra", "websocket"],
    "test": ["uv", "run", "pytest"],
}


def fmt_check() -> int:
    return check("Python",
                 ["uvx", "ruff", "format", "--check", "."],
                 ["uvx", "ruff", "format", "."],
                 cwd=PYTHON_DIR)


def lint() -> int:
    result = subprocess.run(
        ["uvx", "ruff", "check", "."], cwd=PYTHON_DIR, capture_output=True, text=True,
    )
    if result.returncode == 0:
        return 0
    print(result.stdout, result.stderr, sep="", end="", flush=True)
    if os.environ.get("CI"):
        return result.returncode
    subprocess.run(["uvx", "ruff", "check", ".", "--fix"], cwd=PYTHON_DIR)
    print("[WARN] Python lint issues auto-fixed. Stage changes before committing.", flush=True)
    return result.returncode


def clean() -> int:
    import shutil
    shutil.rmtree(PYTHON_DIR / ".venv", ignore_errors=True)
    shutil.rmtree(PYTHON_DIR / ".ruff_cache", ignore_errors=True)
    shutil.rmtree(PYTHON_DIR / ".pytest_cache", ignore_errors=True)
    for egg in PYTHON_DIR.glob("*.egg-info"):
        shutil.rmtree(egg, ignore_errors=True)
    for pycache in PYTHON_DIR.rglob("__pycache__"):
        shutil.rmtree(pycache, ignore_errors=True)
    for pyc in PYTHON_DIR.rglob("*.pyc"):
        pyc.unlink(missing_ok=True)
    return 0


def main() -> None:
    parser = argparse.ArgumentParser(description="Python adapter commands")
    parser.add_argument("command", nargs="?", default="check",
                        choices=["check", "fmt_check", "lint", "clean"] + list(CMDS))
    args = parser.parse_args()
    if args.command == "fmt_check":
        sys.exit(fmt_check())
    if args.command == "lint":
        sys.exit(lint())
    if args.command == "check":
        failed = any([fmt_check() != 0, lint() != 0, run(CMDS["test"], cwd=PYTHON_DIR) != 0])
        sys.exit(1 if failed else 0)
    if args.command == "clean":
        sys.exit(clean())
    if args.command in CMDS:
        sys.exit(run(CMDS[args.command], cwd=PYTHON_DIR))


if __name__ == "__main__":
    main()
