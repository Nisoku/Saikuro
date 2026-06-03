"""Shared dotnet/SDK utilities."""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

DOTNET_CHANNEL = "8.0"


def find_dotnet() -> str | None:
    import shutil
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
    print(f"dotnet not found. Installing .NET SDK {DOTNET_CHANNEL}...", flush=True)
    installer = Path.home() / ".dotnet" / "dotnet-install.sh"
    installer.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        ["curl", "-sSL", "https://dot.net/v1/dotnet-install.sh", "-o", str(installer)],
        check=True,
    )
    installer.chmod(0o755)
    subprocess.run(["bash", str(installer), "--channel", DOTNET_CHANNEL], check=True)
    dotnet = Path.home() / ".dotnet" / "dotnet"
    if dotnet.is_file():
        os.environ["PATH"] = str(dotnet.parent) + os.pathsep + os.environ.get("PATH", "")
        result = subprocess.run([str(dotnet), "--list-sdks"], capture_output=True, text=True)
        if DOTNET_CHANNEL not in result.stdout:
            print(f"warning: installed SDK version may not match channel {DOTNET_CHANNEL}", flush=True)
        print("dotnet installed.", flush=True)
    else:
        print("dotnet installation failed; add ~/.dotnet to your PATH", flush=True)
        sys.exit(1)


def ensure_dotnet_env(env: dict[str, str] | None = None) -> dict[str, str]:
    ensure_dotnet()
    result = dict(env or os.environ)
    dotnet_dir = str(Path.home() / ".dotnet")
    if dotnet_dir not in result.get("PATH", ""):
        result["PATH"] = dotnet_dir + os.pathsep + result.get("PATH", "")
    return result
