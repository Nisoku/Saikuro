"""Python adapter commands."""

import os, subprocess, sys
from pathlib import Path

DIR = Path(__file__).resolve().parents[1] / "adapters" / "python"

CMDS = {
    "setup": ["uv", "sync", "--extra", "dev", "--extra", "websocket"],
    "test": ["uv", "run", "pytest"],
}


def run(cmd: list[str]) -> int:
    return subprocess.run(cmd, cwd=DIR).returncode


def lint() -> int:
    result = subprocess.run(
        ["uvx", "ruff", "check", "."], cwd=DIR, capture_output=True, text=True,
    )
    if result.returncode == 0:
        return 0
    print(result.stdout, result.stderr, sep="", end="", flush=True)
    if os.environ.get("CI"):
        return result.returncode
    subprocess.run(["uvx", "ruff", "check", ".", "--fix"], cwd=DIR)
    print("[WARN] Python lint issues auto-fixed. Stage changes before committing.", flush=True)
    return result.returncode


def main() -> None:
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"
    if cmd == "check":
        sys.exit(sum([lint(), run(CMDS["test"])]))
    elif cmd == "lint":
        sys.exit(lint())
    elif cmd in CMDS:
        sys.exit(run(CMDS[cmd]))
    else:
        print(f"Usage: {sys.argv[0]} <check|lint|{'|'.join(CMDS)}>")
        sys.exit(1)


if __name__ == "__main__":
    main()
