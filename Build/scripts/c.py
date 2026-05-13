"""C adapter commands."""

import subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

CMDS = {
    "build": ["cargo", "build", "-p", "saikuro-c"],
    "test": ["cargo", "test", "-p", "saikuro-c"],
    "check": [],
}


def run(cmd: list[str]) -> int:
    return subprocess.run(cmd, cwd=ROOT).returncode


def main() -> None:
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"
    if cmd == "check":
        exit(sum(run(CMDS[c]) for c in ["build", "test"]))
    if cmd in CMDS:
        exit(run(CMDS[cmd]))
    print(f"Usage: {sys.argv[0]} <{'|'.join(CMDS)}>")
    exit(1)


if __name__ == "__main__":
    main()
