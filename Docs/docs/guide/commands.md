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
| `just rust check`           | fmt + clippy + tests + wasm check (includes `tools/xtask`) |
| `just python setup`         | `uv sync --dev`                     |
| `just python test`          | `pytest`                            |
| `just python check`         | ruff lint + format + pytest         |
| `just typescript setup`     | `npm install`                       |
| `just typescript build`     | Build with tsdown                   |
| `just typescript test`      | `vitest`                            |
| `just typescript typecheck` | `tsc --noEmit`                      |
| `just typescript check`     | eslint + tsc + vitest + tsdown      |
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

## Demo Web App

The `Demo` Vite app and its WASM provider samples are gated together as the `demo` language. This covers the frontend TypeScript plus the C, C++, C#, Python and Rust sources under `Demo/wasm/`.

| Command             | What it does                                                        |
|---------------------|---------------------------------------------------------------------|
| `just demo setup`   | `npm install` in `Demo`                                             |
| `just demo format`  | Prettier + clang-format + ruff + `cargo fmt` + `dotnet format`      |
| `just demo check`   | Format checks + ruff lint + `tsc --noEmit` + `vite build`           |
| `just demo build`   | Build every WASM provider + the Vite bundle                         |
| `just demo dev`     | Build WASM + start the Vite dev server with live rebuilds           |
| `just demo clean`   | Remove `node_modules`, `dist` and generated WASM output             |

`just check` runs the demo gate alongside every other language.

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
| `just demo build`         | Build all WASM modules + Vite bundle     |
| `just demo dev`           | Build WASM + start Vite dev server       |

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
just demo dev              # build WASM + start dev server
```
