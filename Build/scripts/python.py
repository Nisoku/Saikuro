"""Python adapter commands."""

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


def main() -> None:
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"
    if cmd == "fmt_check":
        sys.exit(fmt_check())
    if cmd == "lint":
        sys.exit(lint())
    if cmd == "check":
        failed = any([fmt_check() != 0, lint() != 0, run(CMDS["test"], cwd=PYTHON_DIR) != 0])
        sys.exit(1 if failed else 0)
    if cmd in CMDS:
        sys.exit(run(CMDS[cmd], cwd=PYTHON_DIR))
    print(f"Usage: {sys.argv[0]} <check|fmt_check|lint|{'|'.join(CMDS)}>")
    sys.exit(1)


if __name__ == "__main__":
    main()
