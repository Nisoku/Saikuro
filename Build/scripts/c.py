"""C adapter commands."""

import sys

from shared.constants import BUILD_ROOT, C_DIR
from shared.run import run
from shared.format import check_clang, check

CMDS = {
    "build": ["cargo", "build", "-p", "saikuro-c"],
    "test": ["cargo", "test", "-p", "saikuro-c"],
}


def fmt_check() -> int:
    rc = check_clang([C_DIR / "include"], ["*.h"], cwd=C_DIR)
    rc += check("C crate Rust",
                ["cargo", "fmt", "-p", "saikuro-c", "--", "--check"],
                ["cargo", "fmt", "-p", "saikuro-c"],
                cwd=BUILD_ROOT)
    return rc


def main() -> None:
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"
    if cmd == "fmt_check":
        sys.exit(fmt_check())
    if cmd == "check":
        failed = any([fmt_check() != 0, run(CMDS["build"], cwd=BUILD_ROOT) != 0, run(CMDS["test"], cwd=BUILD_ROOT) != 0])
        sys.exit(1 if failed else 0)
    if cmd in CMDS:
        sys.exit(run(CMDS[cmd], cwd=BUILD_ROOT))
    print(f"Usage: {sys.argv[0]} <check|fmt_check|{'|'.join(CMDS)}>")
    sys.exit(1)


if __name__ == "__main__":
    main()
