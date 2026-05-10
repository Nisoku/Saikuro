"""C++ adapter commands."""

import subprocess, sys
from pathlib import Path

DIR = Path(__file__).resolve().parents[1] / "adapters" / "cpp"

CMDS = {
    "setup": ["cmake", "-S", ".", "-B", "build"],
    "test": ["cmake", "--build", "build", "--target", "saikuro_cpp_header_compile_test"],
    "check": [],
}


def run(cmd: list[str]) -> int:
    return subprocess.run(cmd, cwd=DIR).returncode


def main() -> None:
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"
    if cmd == "check":
        exit(sum(run(CMDS[c]) for c in ["setup", "test"]))
    if cmd in CMDS:
        exit(run(CMDS[cmd]))
    print(f"Usage: {sys.argv[0]} <{'|'.join(CMDS)}>")
    exit(1)


if __name__ == "__main__":
    main()
