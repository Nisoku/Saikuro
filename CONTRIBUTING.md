# Contributing to Saikuro

Thank you for your interest in contributing! This document explains how to get
the project building locally, how to run the tests, and what to keep in mind
when opening a pull request.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Project layout](#project-layout)
- [Quick setup](#quick-setup)
- [Building](#building)
- [Running the tests](#running-the-tests)
- [Code style](#code-style)
- [Opening a pull request](#opening-a-pull-request)

---

## Prerequisites

| Tool            | Minimum version   | Notes                                                 |
|-----------------|-------------------|-------------------------------------------------------|
| Rust toolchain  | 1.75              | Install via [rustup](https://rustup.rs/)              |
| Node.js         | 22 (see `.nvmrc`) | Required for the TypeScript adapter                   |
| Python          | 3.11              | Required for the Python adapter                       |
| uv              | latest            | Python package manager                                |
| just            | latest            | Task runner                                           |
| .NET SDK        | 8.0               | Required for the C# adapter                           |
| C toolchain     | -                 | Required for the C adapter (`clang` or `gcc`)         |
| CMake           | 3.20              | Required for the C++ header tests                     |
| wasm-pack       | latest            | Required for WASM tests (`cargo install wasm-pack`)   |

---

## Project layout

```txt
Build/
  Cargo.toml          # Rust workspace root
  crates/             # Core library crates
    saikuro-core/
    saikuro-schema/
    saikuro-transport/
    saikuro-router/
    saikuro-runtime/
    saikuro-exec/      # Executor abstraction (tokio/wasm/embassy)
    saikuro-codegen/
  tests/              # Rust integration tests
  adapters/
    rust/             # Standalone saikuro crate (end-user API)
    typescript/       # npm package
    python/           # PyPI package (uses uv)
    csharp/           # NuGet package
    c/                # C adapter
  scripts/            # Per-language build/check scripts
    rust.py
    python.py
    typescript.py
    csharp.py
    c.py
    cpp.py
    saikuro_build.py   # Orchestrator (runs all language checks)
Examples/
  rust/math/         # Example Rust provider/client
Docs/                 # Documentation site
```

---

## Quick setup

### Using just

```bash
# Install just (if not installed)
cargo install just

# Clone and setup
git clone https://github.com/Nisoku/Saikuro.git
cd Saikuro
just setup
```

### Dev Container (VS Code)

1. Install the "Dev Containers" extension
2. Open the repo in VS Code
3. Run: `Dev Containers: Reopen in Container`
4. Environment is automatically set up via `.devcontainer/devcontainer.json`

---

## Building

### Rust workspace

```bash
just rust build
```

### TypeScript adapter

```bash
just typescript setup
just typescript build
```

### Python adapter

```bash
just python setup
cd Build/adapters/python && uv run python ...  # or use your IDE
```

### C# adapter

```bash
just csharp check   # builds + runs tests + format check
```

---

## Running the tests

### Everything (recommended)

Runs formatters, linters, typecheckers, and tests for all languages:

```bash
just check
```

Or via the Python orchestrator:

```bash
cd Build && python3 scripts/saikuro_build.py
```

### Per-language checks

```bash
just rust check       # fmt + clippy + tests + wasm compilation check
just python check     # ruff lint + pytest
just typescript check # eslint + tsc + tests + build
just csharp check     # dotnet format + build + tests
just c check          # build + test C adapter
just cpp check        # cmake configure + header compile test
```

### Individual subcommands

```bash
just rust fmt_check
just rust lint
just rust test
just rust wasm_check
just python lint
just python test
just typescript lint
just typescript typecheck
just typescript test
just typescript build
```

---

## Code style

- **Rust**: `cargo fmt` and `cargo clippy -- -D warnings` must pass.
- **TypeScript**: `npm run lint` (ESLint) and `npm run typecheck` (tsc) must pass.
- **Python**: PEP 8; use `ruff` (`uvx ruff check .`).
- **C#**: standard .NET conventions; `dotnet format` must pass.

---

## Opening a pull request

1. Fork the repository and create a feature branch off `main`.
2. Make your changes, ensuring all tests pass locally (`just check`).
3. Add or update tests as appropriate.
4. Update `CHANGELOG.md`.
5. Open a pull request with a clear description of what the change does and why.

Please be respectful and constructive in all interactions. See the
[Code of Conduct](CODE_OF_CONDUCT.md) for our community standards.
