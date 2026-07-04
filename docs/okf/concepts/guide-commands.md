---
type: concept
title: Commands
description: "Saikuro development commands via the just runner"
source: "https://nisoku.org/Saikuro/guide/commands/"
path: /guide/commands/
updated: 2026-07-04
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-07-04T10:28:43.643Z"
---
---
title: "Commands"
description: "Saikuro development commands via the just runner"
---

The project uses [`just`](https://github.com/casey/just) as a command runner. Run `just` from the repo root to list everything.

```bash
cargo install just
```

## Per-Language Commands

| Command                     | What it does                        |
|-----------------------------|-------------------------------------|
| `just rust setup`           | Add `wasm32-unknown-unknown` target |
| `just rust test`            | `cargo test --workspace`            |
| `just rust check`           | fmt + clippy + tests + wasm check   |
| `just python setup`         | `uv sync --dev`                     |
| `just python test`          | `pytest`                            |
| `just python check`         | ruff lint + format + pytest         |
| `just typescript setup`     | `npm install`                       |
| `just typescript build`     | Build with tsup                     |
| `just typescript test`      | `vitest`                            |
| `just typescript typecheck` | `tsc --noEmit`                      |
| `just typescript check`     | eslint + tsc + vitest + tsup        |
| `just csharp setup`         | `dotnet restore`                    |
| `just csharp build`         | `dotnet build -c Release`           |
| `just csharp test`          | `dotnet test -c Release`            |
| `just csharp check`         | dotnet format + build + test        |
| `just c build`              | `cargo build -p saikuro-c`          |
| `just c test`               | `cargo test -p saikuro-c`           |
| `just c check`              | clang-format + build + test         |
| `just cpp setup`            | cmake configure + ensure Emscripten |
| `just cpp test`             | cmake build + ctest                 |
| `just cpp check`            | clang-format + cmake + test         |

## WASM / Demo

| Command                   | What it does                             |
|---------------------------|------------------------------------------|
| `just wasm-rust`          | Build all Rust WASM (runtime + provider) |
| `just wasm-rust-runtime`  | Build only the runtime WASM module       |
| `just wasm-rust-provider` | Build only the Rust provider WASM        |
| `just wasm-c`             | Build C WASM provider                    |
| `just wasm-cpp`           | Build C++ WASM provider                  |
| `just wasm-csharp`        | Build C# WASM provider                   |
| `just wasm-python`        | Build Python WASM provider               |
| `just wasm-all`           | Build every WASM module                  |
| `just web_demo dev`       | Build WASM + start Vite dev server       |
| `just web_demo build`     | Build WASM modules only                  |

## Meta

| Command       | What it does                        |
|---------------|-------------------------------------|
| `just setup`  | Run every language setup            |
| `just format` | Run every formatter (with auto-fix) |
| `just test`   | Run every test suite                |
| `just check`  | Run every check                     |
| `just all`    | `setup` + `check`                   |

## Workflow Examples

After cloning, run once:

```bash
just setup    # install toolchains, restore packages
```

During development:

```bash
just typescript test      # run TypeScript tests
just rust test            # run Rust tests
just test                 # run everything
```

Before committing:

```bash
just check    # lint + format + typecheck + test for all languages
```

Building the WASM demo:

```bash
just wasm-all              # build all WASM modules
just web_demo dev          # build WASM + start dev server
```
