"""Rust workspace + adapter commands."""

import argparse
import sys

from shared.constants import BUILD_ROOT, QEMU_DIR, RUST_DIR
from shared.run import run
from shared.format import check

CMDS = {
    "setup": ["rustup", "target", "add", "wasm32-unknown-unknown"],
    "clean": ["cargo", "clean"],
    "wasm_check": ["cargo", "clippy", "--target", "wasm32-unknown-unknown", "--no-default-features", "--features", "wasm", "--", "-D", "warnings"],
    "test": ["cargo", "test", "--workspace"],
    "adapter_test": ["cargo", "test", "-p", "saikuro"],
}


def fmt_check() -> int:
    rc = check("Rust workspace",
               ["cargo", "fmt", "--all", "--", "--check"],
               ["cargo", "fmt", "--all"],
               cwd=BUILD_ROOT)
    rc += check("Rust adapter",
                ["cargo", "fmt", "--", "--check"],
                ["cargo", "fmt"],
                cwd=RUST_DIR)
    return rc


def lint() -> int:
    return run(["cargo", "clippy", "--workspace", "--", "-D", "warnings"], cwd=BUILD_ROOT) + \
           run(["cargo", "clippy", "--", "-D", "warnings"], cwd=RUST_DIR)


def main() -> None:
    parser = argparse.ArgumentParser(description="Rust workspace + adapter commands")
    parser.add_argument("command", nargs="?", default="check",
                        choices=["check", "fmt_check", "lint"] + list(CMDS))
    args = parser.parse_args()
    if args.command == "fmt_check":
        sys.exit(fmt_check())
    if args.command == "lint":
        sys.exit(lint())
    if args.command == "check":
        failed = any([fmt_check() != 0, lint() != 0,
                      run(CMDS["test"], cwd=BUILD_ROOT) != 0,
                      run(CMDS["wasm_check"], cwd=QEMU_DIR) != 0,
                      run(CMDS["adapter_test"], cwd=BUILD_ROOT) != 0])
        sys.exit(1 if failed else 0)
    if args.command in CMDS:
        cwd = QEMU_DIR if args.command == "wasm_check" else BUILD_ROOT
        sys.exit(run(CMDS[args.command], cwd=cwd))


if __name__ == "__main__":
    main()
