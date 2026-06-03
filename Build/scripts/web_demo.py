#!/usr/bin/env python3
"""Build and run the polyglot web demo."""

from __future__ import annotations

import logging
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Callable, Dict

from shared.constants import DEMO_DIR, WASM_DIR, PUBLIC_WASM, REPO_ROOT, BUILD_ROOT
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
        ems_paths = [str(emsdk_dir), str(emsdk_dir / "upstream" / "emscripten")]
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


def build_rust_component(
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
    
    logger.info(f"Building Rust {component_name} component")
    return run_command(command)


def build_rust_runtime() -> bool:
    """Build the Rust runtime component."""
    return build_rust_component(
        "runtime",
        WASM_DIR / "runtime",
        PUBLIC_WASM / "runtime"
    )


def build_rust_provider() -> bool:
    """Build the Rust provider component."""
    return build_rust_component(
        "provider",
        WASM_DIR / "rust",
        PUBLIC_WASM / "rust"
    )


def build_rust_wasm() -> bool:
    """Build all Rust WASM components."""
    if not build_rust_runtime():
        return False
    return build_rust_provider()


def build_emscripten_component(
    language: str,
    source_file: Path,
    output_file: Path,
    compiler: str = "emcc",
    extra_flags: list[str] | None = None
) -> bool:
    """Build a C/C++ component using Emscripten."""
    output_file.parent.mkdir(parents=True, exist_ok=True)
    
    base_flags = [
        "-O3",
        "-s", "MODULARIZE=1",
        "-s", "EXPORT_ES6=1",
        "-s", "ENVIRONMENT=web",
        "-s", "ALLOW_MEMORY_GROWTH=1",
        "-s", "EXPORTED_RUNTIME_METHODS=stringToUTF8,lengthBytesUTF8,UTF8ToString",
        "-o", str(output_file)
    ]
    
    if language == "cpp":
        base_flags.extend(["-std=c++17"])
    
    if extra_flags:
        base_flags.extend(extra_flags)
    
    command = [compiler, str(source_file)] + base_flags
    
    logger.info(f"Building {language.upper()} WASM component")
    return run_command(command)


def build_c_wasm() -> bool:
    """Build the C WASM component."""
    return build_emscripten_component(
        "c",
        WASM_DIR / "c" / "insight_c.c",
        PUBLIC_WASM / "c" / "insight_c.js",
        extra_flags=[
            "-s", "EXPORTED_FUNCTIONS=_insight_c_stats,_insight_c_free,_malloc,_free"
        ]
    )


def build_cpp_wasm() -> bool:
    """Build the C++ WASM component."""
    return build_emscripten_component(
        "cpp",
        WASM_DIR / "cpp" / "insight_cpp.cpp",
        PUBLIC_WASM / "cpp" / "insight_cpp.js",
        compiler="em++",
        extra_flags=[
            "-s", "EXPORTED_FUNCTIONS=_insight_cpp_ngrams,_insight_cpp_free,_malloc,_free"
        ]
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
        
        return True
    except Exception as e:
        logger.error(f"Failed to copy C# WASM files: {str(e)}")
        return False


def copy_python_files() -> bool:
    """Copy Python files to the public WASM directory."""
    try:
        (PUBLIC_WASM / "python").mkdir(parents=True, exist_ok=True)
        shutil.copy2(
            WASM_DIR / "python" / "insight.py", 
            PUBLIC_WASM / "python" / "insight.py"
        )
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
    """Run the development server."""
    logger.info("Starting development server...")
    return run_command(["node", "dev.mjs"], cwd=DEMO_DIR)


def run_type_check() -> bool:
    """Run TypeScript type checking."""
    logger.info("Running type check...")
    return run_command(["npm", "run", "typecheck"], cwd=DEMO_DIR)


def main() -> int:
    """Main entry point for the web demo script."""
    if len(sys.argv) < 2:
        logger.info("No command specified, defaulting to 'dev'")
        command = "dev"
    else:
        command = sys.argv[1]

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
    if command in command_map:
        logger.info(f"Executing command: {command}")
        success = command_map[command]()
        return 0 if success else 1
    
    # Show usage if command not found
    logger.error("Invalid command")
    logger.info("\nUsage: web_demo.py [setup|build|dev|check|build-c|build-cpp|build-csharp|"
              "build-rust-runtime|build-rust-provider|build-python|build-rust]")
    return 1


if __name__ == "__main__":
    sys.exit(main())
