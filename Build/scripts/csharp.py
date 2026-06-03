"""C# adapter commands."""

import sys

from shared.constants import CSHARP_DIR, CSHARP_SRC, CSHARP_TEST
from shared.run import run
from shared.format import check
from shared.dotnet import ensure_dotnet

CMDS = {
    "setup": ["dotnet", "restore", str(CSHARP_SRC / "Saikuro.csproj")],
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
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"
    if cmd == "check":
        rc_sum = [fmt_check(), run(CMDS["build"], cwd=CSHARP_DIR), run(CMDS["test"], cwd=CSHARP_DIR)]
        sys.exit(0 if all(rc == 0 for rc in rc_sum) else 1)
    if cmd == "fmt_check":
        sys.exit(fmt_check())
    if cmd in CMDS:
        sys.exit(run(CMDS[cmd], cwd=CSHARP_DIR))
    print(f"Usage: {sys.argv[0]} <check|fmt_check|{'|'.join(CMDS)}>")
    sys.exit(1)


if __name__ == "__main__":
    main()
