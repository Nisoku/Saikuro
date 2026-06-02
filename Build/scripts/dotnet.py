"""Shared dotnet/SDK utilities.

Re-exports `DOTNET_CHANNEL` and `DOTNET_INSTALL_SHA256` for use by
callers that want to reference the pinned SDK version.
"""

from __future__ import annotations

import hashlib
import os
import subprocess
import sys
from pathlib import Path

DOTNET_CHANNEL = "8.0"
DOTNET_INSTALL_SHA256 = "082f7685e156738a1b2e2ed8381a621870d4ce8e8c59278034556f05c186eb2e"


def find_dotnet() -> str | None:
    """Return the path to the dotnet binary, or None.

    Checks ``shutil.which`` first, then ``~/.dotnet/dotnet``.
    When found via the latter, prepends ``~/.dotnet`` to ``PATH``
    in the current process environment so that child processes
    can discover it.
    """
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
    """Ensure ``dotnet`` is on ``PATH``, installing if necessary."""
    if find_dotnet():
        return
    print(f"dotnet not found. Installing .NET SDK {DOTNET_CHANNEL}...", flush=True)
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


def ensure_dotnet_env(env: dict[str, str] | None = None) -> dict[str, str]:
    """Return an environment dict with ``~/.dotnet`` on ``PATH``.

    Like :func:`ensure_dotnet` but mutates a *copy* of the provided
    *env* (or ``os.environ``) so callers that build their own subprocess
    environment don't need to repeat the PATH-extension logic.
    """
    ensure_dotnet()
    result = dict(env or os.environ)
    dotnet_dir = str(Path.home() / ".dotnet")
    if dotnet_dir not in result.get("PATH", ""):
        result["PATH"] = dotnet_dir + os.pathsep + result.get("PATH", "")
    return result
