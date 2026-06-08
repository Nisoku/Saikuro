"""C adapter commands."""

import argparse
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
    parser = argparse.ArgumentParser(description="C adapter commands")
    parser.add_argument("command", nargs="?", default="check",
                        choices=["check", "fmt_check"] + list(CMDS))
    args = parser.parse_args()
    if args.command == "fmt_check":
        sys.exit(fmt_check())
    if args.command == "check":
        failed = any([fmt_check() != 0, run(CMDS["build"], cwd=BUILD_ROOT) != 0, run(CMDS["test"], cwd=BUILD_ROOT) != 0])
        sys.exit(1 if failed else 0)
    if args.command in CMDS:
        sys.exit(run(CMDS[args.command], cwd=BUILD_ROOT))


if __name__ == "__main__":
    main()
