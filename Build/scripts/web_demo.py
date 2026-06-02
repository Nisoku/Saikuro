#!/usr/bin/env python3
"""Build and run the polyglot web demo."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

from dotnet import ensure_dotnet, ensure_dotnet_env

SCRIPTS = Path(__file__).resolve().parent
BUILD_ROOT = SCRIPTS.parent
REPO_ROOT = BUILD_ROOT.parent
DEMO = REPO_ROOT / "Demo"
WASM = DEMO / "wasm"
SRC_WASM = DEMO / "src" / "wasm"
PUBLIC_WASM = DEMO / "public" / "wasm"


def run(cmd: list[str], cwd: Path | None = None) -> int:
    """Run a subprocess ensuring emsdk and dotnet paths are available.

    Makes ``emcc``, ``em++`` and ``dotnet`` discoverable even when the
    caller's shell hasn't sourced ``~/.emsdk/emsdk_env.sh`` or added
    ``~/.dotnet`` to ``PATH``.
    """
    env = os.environ.copy()
    emsdk_dir = Path.home() / ".emsdk"
    if emsdk_dir.exists():
        ems_paths = [str(emsdk_dir), str(emsdk_dir / "upstream" / "emscripten")]
        old_path = env.get("PATH", "")
        for p in ems_paths:
            if p not in old_path:
                old_path = f"{p}:{old_path}"
        env["PATH"] = old_path
    env = ensure_dotnet_env(env)
    return subprocess.run(cmd, cwd=cwd or REPO_ROOT, env=env).returncode


def ensure_dirs() -> None:
    (SRC_WASM / "c").mkdir(parents=True, exist_ok=True)
    (SRC_WASM / "cpp").mkdir(parents=True, exist_ok=True)
    (SRC_WASM / "runtime").mkdir(parents=True, exist_ok=True)
    (SRC_WASM / "rust").mkdir(parents=True, exist_ok=True)
    (SRC_WASM / "csharp").mkdir(parents=True, exist_ok=True)
    (PUBLIC_WASM / "python").mkdir(parents=True, exist_ok=True)
    (PUBLIC_WASM / "csharp").mkdir(parents=True, exist_ok=True)


def setup() -> int:
    rc = run(["npm", "install"], cwd=BUILD_ROOT / "adapters" / "typescript")
    if rc != 0:
        return rc
    return run(["npm", "install"], cwd=DEMO)


def build_rust_wasm() -> int:
    rc = run([
        "wasm-pack",
        "build",
        str(WASM / "runtime"),
        "--target",
        "web",
        "--out-dir",
        str(SRC_WASM / "runtime"),
        "--release",
    ])
    if rc != 0:
        return rc
    return run([
        "wasm-pack",
        "build",
        str(WASM / "rust"),
        "--target",
        "web",
        "--out-dir",
        str(SRC_WASM / "rust"),
        "--release",
    ])


def build_c_wasm() -> int:
    return run([
        "emcc",
        str(WASM / "c" / "insight_c.c"),
        "-O3",
        "-s",
        "MODULARIZE=1",
        "-s",
        "EXPORT_ES6=1",
        "-s",
        "ENVIRONMENT=web",
        "-s",
        "ALLOW_MEMORY_GROWTH=1",
        "-s",
        "EXPORTED_FUNCTIONS=_insight_c_stats,_insight_c_free,_malloc,_free",
        "-s",
        "EXPORTED_RUNTIME_METHODS=stringToUTF8,lengthBytesUTF8,UTF8ToString",
        "-o",
        str(SRC_WASM / "c" / "insight_c.js"),
    ])


def build_cpp_wasm() -> int:
    return run([
        "em++",
        str(WASM / "cpp" / "insight_cpp.cpp"),
        "-O3",
        "-std=c++17",
        "-s",
        "MODULARIZE=1",
        "-s",
        "EXPORT_ES6=1",
        "-s",
        "ENVIRONMENT=web",
        "-s",
        "ALLOW_MEMORY_GROWTH=1",
        "-s",
        "EXPORTED_FUNCTIONS=_insight_cpp_ngrams,_insight_cpp_free,_malloc,_free",
        "-s",
        "EXPORTED_RUNTIME_METHODS=stringToUTF8,lengthBytesUTF8,UTF8ToString",
        "-o",
        str(SRC_WASM / "cpp" / "insight_cpp.js"),
    ])


def build_csharp_wasm() -> int:
    ensure_dotnet()
    rc = run([
        "dotnet",
        "publish",
        str(WASM / "csharp" / "InsightLab"),
        "-c",
        "Release",
        "-p:RuntimeIdentifier=browser-wasm",
        "-p:SelfContained=true",
        "-p:WasmBuildNative=true",
    ])
    if rc != 0:
        return rc

    publish_dir = WASM / "csharp" / "InsightLab" / "bin" / "Release" / "net8.0" / "browser-wasm" / "publish"
    if not publish_dir.exists():
        print(f"publish output not found: {publish_dir}")
        return 1

    shutil.rmtree(SRC_WASM / "csharp", ignore_errors=True)
    shutil.copytree(publish_dir, SRC_WASM / "csharp", dirs_exist_ok=True)

    shutil.rmtree(PUBLIC_WASM / "csharp", ignore_errors=True)
    shutil.copytree(publish_dir, PUBLIC_WASM / "csharp", dirs_exist_ok=True)
    return 0


def copy_python() -> int:
    src = WASM / "python" / "insight.py"
    dst = PUBLIC_WASM / "python" / "insight.py"
    shutil.copy2(src, dst)
    return 0


def build_all() -> int:
    ensure_dirs()
    for step in [build_rust_wasm, build_c_wasm, build_cpp_wasm, build_csharp_wasm, copy_python]:
        rc = step()
        if rc != 0:
            return rc
    return run(["npm", "run", "build"], cwd=DEMO)


def dev() -> int:
    ensure_dirs()
    for step in [build_rust_wasm, build_c_wasm, build_cpp_wasm, build_csharp_wasm, copy_python]:
        rc = step()
        if rc != 0:
            return rc
    return run(["npm", "run", "dev"], cwd=DEMO)


def main() -> int:
    cmd = sys.argv[1] if len(sys.argv) > 1 else "dev"

    if cmd == "setup":
        return setup()
    if cmd == "build":
        return build_all()
    if cmd == "dev":
        return dev()
    if cmd == "check":
        return run(["npm", "run", "typecheck"], cwd=DEMO)

    print("Usage: web_demo.py [setup|build|dev|check]")
    return 1


if __name__ == "__main__":
    sys.exit(main())
