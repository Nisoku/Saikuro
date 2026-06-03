"""TypeScript adapter commands."""

import sys

from shared.constants import TYPESCRIPT_DIR
from shared.run import run
from shared.format import check

CMDS = {
    "setup": ["npm", "install"],
    "typecheck": ["npm", "run", "typecheck"],
    "build": ["npm", "run", "build"],
    "test": ["npm", "test"],
}


def fmt_check() -> int:
    return check("TypeScript",
                 ["npm", "run", "format:check"],
                 ["npm", "run", "format"],
                 cwd=TYPESCRIPT_DIR)


def lint() -> int:
    result = __import__("subprocess").run(
        ["npm", "run", "lint"], cwd=TYPESCRIPT_DIR, capture_output=True, text=True,
    )
    if result.returncode == 0:
        return 0
    print(result.stdout, result.stderr, sep="", end="", flush=True)
    __import__("subprocess").run(["npm", "run", "lint:fix"], cwd=TYPESCRIPT_DIR)
    print("[WARN] TypeScript lint issues auto-fixed. Stage changes before committing.", flush=True)
    return result.returncode


def main() -> None:
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"
    if cmd == "check":
        steps = [
            ("lint", lint),
            ("typecheck", lambda: run(CMDS["typecheck"], cwd=TYPESCRIPT_DIR)),
            ("test", lambda: run(CMDS["test"], cwd=TYPESCRIPT_DIR)),
            ("build", lambda: run(CMDS["build"], cwd=TYPESCRIPT_DIR)),
        ]
        for name, step in steps:
            code = step()
            if code != 0:
                print(f"[FAIL] {name} exited with code {code}", flush=True)
                sys.exit(code)
        sys.exit(0)
    if cmd == "fmt_check":
        sys.exit(fmt_check())
    if cmd == "lint":
        sys.exit(lint())
    if cmd in CMDS:
        sys.exit(run(CMDS[cmd], cwd=TYPESCRIPT_DIR))
    print(f"Usage: {sys.argv[0]} <check|fmt_check|lint|{'|'.join(CMDS)}>")
    sys.exit(1)


if __name__ == "__main__":
    main()
