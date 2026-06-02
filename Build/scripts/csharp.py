"""C# adapter commands."""

import subprocess, sys
from pathlib import Path

from dotnet import ensure_dotnet

DIR = Path(__file__).resolve().parents[1] / "adapters" / "csharp" / "Saikuro"
SRC = DIR / "src"
TEST = DIR / "tests"

CMDS = {
    "setup": ["dotnet", "restore", str(SRC / "Saikuro.csproj")],
    "build": ["dotnet", "build", str(SRC / "Saikuro.csproj"), "-c", "Release"],
    "test": ["dotnet", "test", str(TEST / "Saikuro.Tests.csproj"), "-c", "Release"],
}


def run(cmd: list[str]) -> int:
    if cmd[0] == "dotnet":
        ensure_dotnet()
    return subprocess.run(cmd, cwd=DIR).returncode


def fmt_check() -> int:
    ensure_dotnet()
    project = str(SRC / "Saikuro.csproj")
    result = subprocess.run(
        ["dotnet", "format", project, "--verify-no-changes"],
        cwd=DIR, capture_output=True, text=True,
    )
    if result.returncode == 0:
        return 0
    print(result.stdout, result.stderr, sep="", end="", flush=True)
    subprocess.run(["dotnet", "format", project], cwd=DIR)
    print("[WARN] C# format issues auto-fixed. Stage the changes before committing.", flush=True)
    return result.returncode


def main() -> None:
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"
    if cmd == "check":
        rc_sum = [fmt_check(), run(CMDS["build"]), run(CMDS["test"])]
        sys.exit(0 if all(rc == 0 for rc in rc_sum) else 1)
    elif cmd == "fmt_check":
        sys.exit(fmt_check())
    elif cmd in CMDS:
        sys.exit(run(CMDS[cmd]))
    else:
        print(f"Usage: {sys.argv[0]} <check|fmt_check|{'|'.join(CMDS)}>")
        sys.exit(1)


if __name__ == "__main__":
    main()
