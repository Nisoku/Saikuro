import subprocess
from pathlib import Path

from shared.constants import REPO_ROOT

StrPath = str | Path


def run(cmd: list[str], cwd: StrPath = REPO_ROOT) -> int:
    return subprocess.run(cmd, cwd=str(cwd) if isinstance(cwd, Path) else cwd).returncode


def run_capture(cmd: list[str], cwd: StrPath = REPO_ROOT) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, cwd=str(cwd) if isinstance(cwd, Path) else cwd, capture_output=True, text=True)
