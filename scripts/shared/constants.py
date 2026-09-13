from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPTS = REPO_ROOT / "scripts"
BUILD_ROOT = REPO_ROOT / "Build"

RUST_DIR = BUILD_ROOT / "adapters" / "rust"
PYTHON_DIR = BUILD_ROOT / "adapters" / "python"
TYPESCRIPT_DIR = BUILD_ROOT / "adapters" / "typescript"
CSHARP_DIR = BUILD_ROOT / "adapters" / "csharp" / "Saikuro"
C_DIR = BUILD_ROOT / "adapters" / "c"
CPP_DIR = BUILD_ROOT / "adapters" / "cpp"

CSHARP_SRC = CSHARP_DIR / "src"
CSHARP_TEST = CSHARP_DIR / "tests"

C_FMT_DIRS = [C_DIR / "include"]
CPP_FMT_DIRS = [CPP_DIR / "src", CPP_DIR / "include", CPP_DIR / "tests"]

QEMU_DIR = REPO_ROOT / "Tests"

DEMO_DIR = REPO_ROOT / "Demo"
WASM_DIR = DEMO_DIR / "wasm"
PUBLIC_WASM = DEMO_DIR / "public" / "wasm"
