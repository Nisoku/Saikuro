"""Rust workspace + adapter commands."""

import sys

from shared.constants import BUILD_ROOT, RUST_DIR
from shared.run import run
from shared.format import check

CMDS = {
    "setup": ["rustup", "target", "add", "wasm32-unknown-unknown"],
    "wasm_check": ["cargo", "clippy", "--target", "wasm32-unknown-unknown", "-p", "saikuro-tests", "--", "-D", "warnings"],
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
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"
    if cmd == "fmt_check":
        sys.exit(fmt_check())
    if cmd == "lint":
        sys.exit(lint())
    if cmd == "check":
        sys.exit(sum([fmt_check(), lint(), run(CMDS["test"], cwd=BUILD_ROOT),
                  run(CMDS["wasm_check"], cwd=BUILD_ROOT), run(CMDS["adapter_test"], cwd=BUILD_ROOT)]))
    if cmd in CMDS:
        sys.exit(run(CMDS[cmd], cwd=BUILD_ROOT))
    print(f"Usage: {sys.argv[0]} <check|fmt_check|lint|{'|'.join(CMDS)}>")
    sys.exit(1)


if __name__ == "__main__":
    main()
