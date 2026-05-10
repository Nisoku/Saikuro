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
    return result.returncode


def main() -> None:
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"
    if cmd == "check":
        steps = [("lint", lint), ("typecheck", lambda: run(CMDS["typecheck"])), ("test", lambda: run(CMDS["test"])), ("build", lambda: run(CMDS["build"]))]
        for name, step in steps:
            code = step()
            if code != 0:
                print(f"[FAIL] {name} exited with code {code}", flush=True)
                sys.exit(code)
        sys.exit(0)
    elif cmd == "lint":
        sys.exit(lint())
    elif cmd in CMDS:
        sys.exit(run(CMDS[cmd]))
    else:
        print(f"Usage: {sys.argv[0]} <check|lint|{'|'.join(CMDS)}>")
        sys.exit(1)


if __name__ == "__main__":
    main()
