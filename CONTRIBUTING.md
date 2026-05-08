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

| Tool            | Minimum version   | Notes                                    |
|-----------------|-------------------|------------------------------------------|
| Rust toolchain  | 1.75              | Install via [rustup](https://rustup.rs/) |
| Node.js         | 22 (see `.nvmrc`) | Required for the TypeScript adapter      |
| Python          | 3.11              | Required for the Python adapter          |
| uv              | latest            | Python package manager (replaces pip)    |
| just (optional) | latest            | Task runner (alternative: make)          |
| .NET SDK        | 8.0               | Required for the C# adapter (optional)   |

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
Examples/
  rust/math/         # Example Rust provider/client
Docs/                 # Documentation site
```

---

## Quick setup

### Option 1: Using just

```bash
# Install just (if not installed)
cargo install just

# Clone and setup
git clone https://github.com/Nisoku/Saikuro.git
cd Saikuro
just setup
```

### Option 2: Using make

```bash
git clone https://github.com/Nisoku/Saikuro.git
cd Saikuro
make setup
```

### Option 3: Manual setup

```bash
git clone https://github.com/Nisoku/Saikuro.git
cd Saikuro

# Install uv (Python package manager)
curl -LsSf https://astral.sh/uv/install.sh | sh

# Rust workspace
cd Build && cargo build --workspace

# Python adapter (using uv)
cd Build/adapters/python
uv venv && uv pip install -e ".[dev,websocket]"

# TypeScript adapter
cd Build/adapters/typescript
npm install
```

### Option 4: Dev Container (VS Code)

1. Install the "Dev Containers" extension
2. Open the repo in VS Code
3. Run: `Dev Containers: Reopen in Container`
4. Environment is automatically set up via `.devcontainer/devcontainer.json`

---

## Building

### Rust workspace

```bash
cd Build
cargo build --workspace
```

### TypeScript adapter

```bash
cd Build/adapters/typescript
npm ci
npm run build
```

### Python adapter (using uv)

```bash
cd Build/adapters/python
uv pip install -e ".[dev,websocket]"
```

### C# adapter

```bash
cd Build/adapters/csharp/Saikuro/src
dotnet build
```

---

## Running the tests

### All tests (recommended)

```bash
# Using just
just check

# Using make
make check

# Using build script directly
cd Build && python3 scripts/saikuro_build.py all
```

### Rust

```bash
# All workspace crates + integration tests
cd Build
cargo test --workspace

# Integration tests only
cargo test -p saikuro-tests
```

### TypeScript

```bash
cd Build/adapters/typescript
npm test
```

### Python (using uv)

```bash
cd Build/adapters/python
uv run pytest
```

### C#

```bash
cd Build/adapters/csharp/Saikuro
dotnet test
```

---

## Code style

- **Rust**: `cargo fmt` and `cargo clippy -- -D warnings` must pass.
- **TypeScript**: `npm run lint` (ESLint) and `npm run typecheck` (tsc) must pass.
- **Python**: PEP 8; use `ruff` (`uv run ruff check .`).
- **C#**: standard .NET conventions; `dotnet format` is acceptable but `csharpier` is preferred.

---

## Opening a pull request

1. Fork the repository and create a feature branch off `main`.
2. Make your changes, ensuring all tests pass locally (`just check` or `make check`).
3. Add or update tests as appropriate.
4. Update `CHANGELOG.md`.
5. Open a pull request with a clear description of what the change does and why.

Please be respectful and constructive in all interactions. See the
[Code of Conduct](CODE_OF_CONDUCT.md) for our community standards.
