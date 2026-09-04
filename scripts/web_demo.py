#!/usr/bin/env python3
"""Build and run the polyglot web demo."""

from __future__ import annotations

import argparse
import logging
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Callable, Dict

from shared.constants import C_DIR, CPP_DIR, CSHARP_DIR, DEMO_DIR, WASM_DIR, PUBLIC_WASM, REPO_ROOT, BUILD_ROOT
from shared.dotnet import ensure_dotnet, ensure_dotnet_env

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(levelname)s - %(message)s",
    handlers=[logging.StreamHandler()]
)
logger = logging.getLogger(__name__)


def run_command(
    command: list[str], 
    cwd: Path | None = None,
    env: Dict[str, str] | None = None
) -> bool:
    """Run a shell command with error handling and logging."""
    env = env or os.environ.copy()

    emsdk_dir = Path.home() / ".emsdk"
    if emsdk_dir.exists():
        ems_paths = [
            str(emsdk_dir / "upstream" / "bin"),
        ]
        old_path = env.get("PATH", "")
        for path in ems_paths:
            if path not in old_path:
                old_path = f"{path}:{old_path}"
        env["PATH"] = old_path

    env = ensure_dotnet_env(env)
    
    try:
        result = subprocess.run(
            command, 
            cwd=cwd or REPO_ROOT,
            env=env,
            check=True,
            capture_output=True,
            text=True
        )
        logger.info(f"Command succeeded: {' '.join(command)}")
        return True
    except subprocess.CalledProcessError as e:
        logger.error(f"Command failed: {' '.join(command)}")
        logger.error(f"Error: {e.stderr}")
        return False


def ensure_directories() -> None:
    """Ensure all required directories exist."""
    directories = [
        PUBLIC_WASM / "c",
        PUBLIC_WASM / "cpp",
        PUBLIC_WASM / "runtime",
        PUBLIC_WASM / "rust",
        PUBLIC_WASM / "python",
        PUBLIC_WASM / "csharp"
    ]
    
    for directory in directories:
        directory.mkdir(parents=True, exist_ok=True)


def setup_dependencies() -> bool:
    """Install npm dependencies for TypeScript adapters and demo."""
    steps = [
        ("Installing TypeScript adapter dependencies", 
         ["npm", "install"], 
         BUILD_ROOT / "adapters" / "typescript"),
        ("Installing demo dependencies", 
         ["npm", "install"], 
         DEMO_DIR)
    ]
    
    for description, command, cwd in steps:
        logger.info(description)
        if not run_command(command, cwd):
            logger.error(f"Failed to {description.lower()}")
            return False
    
    return True


def build_wasm_pack_component(
    component_name: str, 
    source_path: Path, 
    output_path: Path
) -> bool:
    """Build a Rust component using wasm-pack."""
    output_path.mkdir(parents=True, exist_ok=True)
    
    command = [
        "wasm-pack",
        "build",
        str(source_path),
        "--target", "web",
        "--out-dir", str(output_path),
        "--release"
    ]
    
    logger.info(f"Building {component_name} component with wasm-pack")
    result = run_command(command)
    if result:
        gitignore = output_path / ".gitignore"
        if gitignore.exists():
            gitignore.unlink()
            logger.info(f"Removed wasm-pack .gitignore from {output_path}")
    return result


def build_rust_runtime() -> bool:
    """Build the Rust runtime component."""
    return build_wasm_pack_component(
        "runtime",
        WASM_DIR / "runtime",
        PUBLIC_WASM / "runtime"
    )


def build_rust_provider() -> bool:
    """Build the Rust provider component."""
    return build_wasm_pack_component(
        "provider",
        WASM_DIR / "rust",
        PUBLIC_WASM / "rust"
    )


def build_rust_wasm() -> bool:
    """Build all Rust WASM components."""
    if not build_rust_runtime():
        return False
    return build_rust_provider()


def build_c_wasm() -> bool:
    """Build the C WASM component via wasm-pack + cc crate."""
    return build_wasm_pack_component(
        "C",
        WASM_DIR / "c",
        PUBLIC_WASM / "c"
    )


def build_cpp_wasm() -> bool:
    """Build the C++ WASM component via wasm-pack + cc crate."""
    return build_wasm_pack_component(
        "C++",
        WASM_DIR / "cpp",
        PUBLIC_WASM / "cpp"
    )


def build_csharp_wasm() -> bool:
    """Build the C# WASM component."""
    ensure_dotnet()
    
    # Publish the C# project
    publish_command = [
        "dotnet", "publish",
        str(WASM_DIR / "csharp" / "InsightLab"),
        "-c", "Release",
        "-p:RuntimeIdentifier=browser-wasm",
        "-p:SelfContained=true",
        "-p:WasmBuildNative=true"
    ]
    
    if not run_command(publish_command):
        return False
    
    # Determine the publish directory
    bundle_dir = WASM_DIR / "csharp" / "InsightLab" / "bin" / "Release" / "net8.0" / "browser-wasm" / "AppBundle" / "_framework"
    fallback_dir = WASM_DIR / "csharp" / "InsightLab" / "bin" / "Release" / "net8.0" / "browser-wasm" / "publish"
    publish_dir = bundle_dir if bundle_dir.exists() else fallback_dir
    
    if not publish_dir.exists():
        logger.error(f"Publish output not found: {publish_dir}")
        return False
    
    # Copy to destination directories
    try:
        shutil.rmtree(PUBLIC_WASM / "csharp", ignore_errors=True)
        shutil.copytree(publish_dir, PUBLIC_WASM / "csharp", dirs_exist_ok=True)

        # Copy the Saikuro BroadcastChannel JS module alongside the dotnet runtime
        bc_js = CSHARP_DIR / "src" / "BroadcastChannel" / "wwwroot" / "Saikuro.BroadcastChannel.js"
        if bc_js.exists():
            shutil.copy2(bc_js, PUBLIC_WASM / "csharp" / "Saikuro.BroadcastChannel.js")
        
        return True
    except Exception as e:
        logger.error(f"Failed to copy C# WASM files: {str(e)}")
        return False


def build_python_wheel() -> bool:
    """Build the saikuro Python wheel (uv) and a pure-Python msgpack wheel (pip)."""
    python_dir = BUILD_ROOT / "adapters" / "python"
    dist_dir = PUBLIC_WASM / "python"
    dist_dir.mkdir(parents=True, exist_ok=True)

    import tempfile
    import glob as glob_module
    with tempfile.TemporaryDirectory() as tmp:
        # Build pure-Python msgpack wheel for Pyodide (msgpack on PyPI only ships
        # platform wheels with C extensions, which micropip cannot install).
        logger.info("Building msgpack pure-Python wheel")
        result = subprocess.run(
            [sys.executable, "-m", "pip", "wheel", "--no-deps",
             "--no-binary", "msgpack", "--wheel-dir", tmp, "msgpack==1.2.1"],
            capture_output=True, text=True,
            env={**os.environ, "MSGPACK_PUREPYTHON": "1"},
        )
        if result.returncode != 0:
            logger.error("msgpack wheel build failed:\n%s", result.stderr)
            return False
        for w in glob_module.glob(os.path.join(tmp, "msgpack-*.whl")):
            dst = dist_dir / os.path.basename(w)
            shutil.copy2(w, dst)
            logger.info("Copied msgpack wheel: %s", dst.name)

        # Build saikuro wheel
        result = subprocess.run(
            ["uv", "build", "--wheel", "--out-dir", tmp],
            cwd=python_dir,
            capture_output=True, text=True,
        )
        if result.returncode != 0:
            logger.error("uv build failed:\n%s", result.stderr)
            return False

        saikuro_wheels = [
            w for w in glob_module.glob(os.path.join(tmp, "*.whl"))
            if "saikuro" in os.path.basename(w)
        ]
        if not saikuro_wheels:
            logger.error("no saikuro wheel produced by uv build")
            return False

        wheel_src = saikuro_wheels[0]
        wheel_dst = dist_dir / os.path.basename(wheel_src)
        shutil.copy2(wheel_src, wheel_dst)
        logger.info("Built Python wheel: %s (%d bytes)", wheel_dst, wheel_dst.stat().st_size)

        # Remove old hand-rolled wheel if present
        old = dist_dir / "saikuro-0.1.0-py3-none-any.whl"
        if old.exists() and old.name != wheel_dst.name:
            old.unlink()

    return True


def copy_python_files() -> bool:
    """Copy Python files and wheel to the public WASM directory."""
    try:
        (PUBLIC_WASM / "python").mkdir(parents=True, exist_ok=True)
        shutil.copy2(
            WASM_DIR / "python" / "insight.py", 
            PUBLIC_WASM / "python" / "insight.py"
        )
        if not build_python_wheel():
            return False
        logger.info("Copied Python files successfully")
        return True
    except Exception as e:
        logger.error(f"Failed to copy Python files: {str(e)}")
        return False


def build_all() -> bool:
    """Build all components and the demo."""
    ensure_directories()
    
    build_steps = [
        ("Rust WASM", build_rust_wasm),
        ("C WASM", build_c_wasm),
        ("C++ WASM", build_cpp_wasm),
        ("C# WASM", build_csharp_wasm),
        ("Python files", copy_python_files)
    ]
    
    for description, step in build_steps:
        logger.info(f"Building {description}...")
        if not step():
            logger.error(f"Failed to build {description}")
            return False
    
    # Build the demo
    logger.info("Building demo...")
    return run_command(["npm", "run", "build"], cwd=DEMO_DIR)


def run_dev_server() -> bool:
    """Run the development server with live output."""
    logger.info("Starting development server...")
    try:
        subprocess.run(["node", "dev.mjs"], cwd=DEMO_DIR, check=True)
        return True
    except subprocess.CalledProcessError:
        return False


def run_type_check() -> bool:
    """Run TypeScript type checking."""
    logger.info("Running type check...")
    return run_command(["npm", "run", "typecheck"], cwd=DEMO_DIR)


def main() -> int:
    """Main entry point for the web demo script."""
    parser = argparse.ArgumentParser(description="Build and run the polyglot web demo")
    parser.add_argument("command", nargs="?", default="dev",
                        choices=["setup", "build", "dev", "check",
                                 "build-c", "build-cpp", "build-csharp",
                                 "build-rust-runtime", "build-rust-provider",
                                 "build-python", "build-rust"])
    args = parser.parse_args()
    command = args.command

    # Define command mappings
    command_map = {
        "setup": setup_dependencies,
        "build": build_all,
        "dev": run_dev_server,
        "check": run_type_check,
        "build-c": build_c_wasm,
        "build-cpp": build_cpp_wasm,
        "build-csharp": build_csharp_wasm,
        "build-rust-runtime": build_rust_runtime,
        "build-rust-provider": build_rust_provider,
        "build-python": copy_python_files,
        "build-rust": build_rust_wasm
    }

    # Execute the command
    logger.info(f"Executing command: {command}")
    success = command_map[command]()
    return 0 if success else 1


if __name__ == "__main__":
    sys.exit(main())
