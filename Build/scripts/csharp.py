"""C# adapter commands."""

import hashlib, os, shutil, subprocess, sys
from pathlib import Path

# Pinned SHA256 of https://dot.net/v1/dotnet-install.sh.
# Update when upgrading the .NET SDK version.
DOTNET_CHANNEL = "8.0"
DOTNET_INSTALL_SHA256 = "102a6849303713f15462bb28eb10593bf874bbeec17122e0522f10a3b57ce442"

DIR = Path(__file__).resolve().parents[1] / "adapters" / "csharp" / "Saikuro"
SRC = DIR / "src"
TEST = DIR / "tests"

CMDS = {
    "setup": ["dotnet", "restore", str(SRC / "Saikuro.csproj")],
    "build": ["dotnet", "build", str(SRC / "Saikuro.csproj"), "-c", "Release"],
    "test": ["dotnet", "test", str(TEST / "Saikuro.Tests.csproj"), "-c", "Release"],
}


def find_dotnet() -> str | None:
    found = shutil.which("dotnet")
    if found:
        return found
    home = Path.home() / ".dotnet" / "dotnet"
    if home.is_file():
        os.environ["PATH"] = str(home.parent) + os.pathsep + os.environ.get("PATH", "")
        return str(home)
    return None


def ensure_dotnet() -> None:
    if find_dotnet():
        return
    print("dotnet not found. Installing .NET SDK 8.0...", flush=True)
    installer = Path.home() / ".dotnet" / "dotnet-install.sh"
    installer.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        ["curl", "-sSL", "https://dot.net/v1/dotnet-install.sh", "-o", str(installer)],
        check=True,
    )
    actual = hashlib.sha256(installer.read_bytes()).hexdigest()
    if actual != DOTNET_INSTALL_SHA256:
        print(f"SHA256 mismatch: expected {DOTNET_INSTALL_SHA256}, got {actual}", flush=True)
        sys.exit(1)
    installer.chmod(0o755)
    subprocess.run(["bash", str(installer), "--channel", DOTNET_CHANNEL], check=True)
    dotnet = Path.home() / ".dotnet" / "dotnet"
    if dotnet.is_file():
        os.environ["PATH"] = str(dotnet.parent) + os.pathsep + os.environ.get("PATH", "")
        print("dotnet installed.", flush=True)
    else:
        print("dotnet installation failed; add ~/.dotnet to your PATH", flush=True)
        sys.exit(1)


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
