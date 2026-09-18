# Polyglot Insight Lab

A browser demo that runs a Saikuro runtime in WebAssembly and fans out to
multiple WASM modules written in different languages.

## What it does

Pipeline:

Input -> C -> C++ -> Rust -> C# -> Python -> Frontend

- C (WASM): character stats
- C++ (WASM): tokenization + n-gram counts
- Rust (WASM): sentiment scoring
- C# (WASM): summary logic
- Python (Pyodide): visualization prep
- TypeScript (Vite): orchestration + UI

All stage boundaries are Saikuro calls over the `wasm-host` transport.

## Run

From repo root:

```bash
just demo dev
```

The script builds the WASM modules, copies artifacts, and starts Vite.

## Manual steps

```bash
cd Demo
npm install
npm run dev
```

WASM build (if you want to run by hand; from repo root, as the `xtask` alias
resolves its manifest path relative to the current directory):

```bash
cargo xtask lang demo build
```

Formatting, linting and the CI gate run through the same `demo` language:

```bash
just demo format   # prettier + clang-format + ruff + cargo fmt + dotnet format
just demo check    # format checks + ruff lint + tsc + vite build
just demo clean    # remove node_modules, dist, generated WASM
```

## Tooling requirements

- Rust toolchain + `wasm-pack`
- Emscripten (`emcc` + `em++`) for C/C++
- .NET 8 SDK for C# WASM
- Node 22+
