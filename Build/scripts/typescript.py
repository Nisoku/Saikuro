"""TypeScript adapter commands."""

import subprocess, sys
from pathlib import Path

DIR = Path(__file__).resolve().parents[1] / "adapters" / "typescript"

CMDS = {
    "setup": ["npm", "install"],
    "typecheck": ["npm", "run", "typecheck"],
    "build": ["npm", "run", "build"],
    "test": ["npm", "test"],
}


def run(cmd: list[str]) -> int:
    return subprocess.run(cmd, cwd=DIR).returncode


def lint() -> int:
    result = subprocess.run(
        ["npm", "run", "lint"], cwd=DIR, capture_output=True, text=True,
    )
    if result.returncode == 0:
        return 0
    print(result.stdout, result.stderr, sep="", end="", flush=True)
    subprocess.run(["npm", "run", "lint:fix"], cwd=DIR)
    print("[WARN] TypeScript lint issues auto-fixed. Stage changes before committing.", flush=True)
    return 0


def main() -> None:
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"
    if cmd == "check":
        exit(sum([lint(), run(CMDS["typecheck"]), run(CMDS["test"]), run(CMDS["build"])]))
    if cmd == "lint":
        lint()
        return
    if cmd in CMDS:
        exit(run(CMDS[cmd]))
    print(f"Usage: {sys.argv[0]} <check|lint|{'|'.join(CMDS)}>")
    exit(1)


if __name__ == "__main__":
    main()
