"""C# adapter commands."""

import argparse
import sys

from shared.constants import CSHARP_DIR, CSHARP_SRC, CSHARP_TEST
from shared.run import run
from shared.format import check
from shared.dotnet import ensure_dotnet

CMDS = {
    "setup": ["dotnet", "restore", str(CSHARP_SRC / "Saikuro.csproj")],
    "clean": ["rm", "-rf", "src/bin", "src/obj", "tests/bin", "tests/obj"],
    "build": ["dotnet", "build", str(CSHARP_SRC / "Saikuro.csproj"), "-c", "Release"],
    "test": ["dotnet", "test", str(CSHARP_TEST / "Saikuro.Tests.csproj"), "-c", "Release"],
}


def fmt_check() -> int:
    ensure_dotnet()
    project = str(CSHARP_SRC / "Saikuro.csproj")
    return check("C#",
                 ["dotnet", "format", project, "--verify-no-changes"],
                 ["dotnet", "format", project],
                 cwd=CSHARP_DIR)


def main() -> None:
    parser = argparse.ArgumentParser(description="C# adapter commands")
    parser.add_argument("command", nargs="?", default="check",
                        choices=["check", "fmt_check"] + list(CMDS))
    args = parser.parse_args()
    if args.command == "check":
        rc_sum = [fmt_check(), run(CMDS["build"], cwd=CSHARP_DIR), run(CMDS["test"], cwd=CSHARP_DIR)]
        sys.exit(0 if all(rc == 0 for rc in rc_sum) else 1)
    if args.command == "fmt_check":
        sys.exit(fmt_check())
    if args.command in CMDS:
        sys.exit(run(CMDS[args.command], cwd=CSHARP_DIR))


if __name__ == "__main__":
    main()
