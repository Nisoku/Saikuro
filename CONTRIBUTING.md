# Contributing to Saikuro

Thank you for your interest in contributing! This document explains how to get
the project building locally, how to run the tests, and what to keep in mind
when opening a pull request.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Project layout](#project-layout)
- [Building](#building)
- [Running the tests](#running-the-tests)
- [Code style](#code-style)
- [Opening a pull request](#opening-a-pull-request)

---

## Prerequisites

| Tool           | Minimum version   | Notes                                    |
| -------------- | ----------------- | ---------------------------------------- |
| Rust toolchain | 1.75              | Install via [rustup](https://rustup.rs/) |
| Node.js        | 22 (see `.nvmrc`) | Required for the TypeScript adapter      |
| Python         | 3.11              | Required for the Python adapter          |
| .NET SDK       | 8.0               | Required for the C# adapter              |

---

## Project layout

```
Build/
  Cargo.toml          # Rust workspace root
  crates/             # Core library crates
    saikuro-core/
    saikuro-schema/
    saikuro-transport/
    saikuro-router/
    saikuro-runtime/
    saikuro-codegen/
    saikuro-runtime-bin/
  tests/              # Rust integration tests
  adapters/
    rust/             # Standalone saikuro crate (end-user API)
    typescript/       # npm package
    python/           # PyPI package
    csharp/           # NuGet package
Docs/                 # Documentation site
```

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

### Python adapter

```bash
cd Build/adapters/python
pip install -e ".[dev]"
```

### C# adapter

```bash
cd Build/adapters/csharp/Saikuro/src
dotnet build
```

---

## Running the tests

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

### Python

```bash
cd Build/adapters/python
pytest
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
- **Python**: PEP 8; use `ruff`.
- **C#**: standard .NET conventions; `dotnet format` is acceptable but `csharpier` is preferred.

---

## Opening a pull request

1. Fork the repository and create a feature branch off `main`.
2. Make your changes, ensuring all tests pass locally.
3. Add or update tests as appropriate.
4. Update `CHANGELOG.md`.
5. Open a pull request with a clear description of what the change does and why.

Please be respectful and constructive in all interactions. See the
[Code of Conduct](CODE_OF_CONDUCT.md) for our community standards.
