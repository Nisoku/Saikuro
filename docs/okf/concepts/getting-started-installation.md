---
type: concept
title: Installation
description: "Install Saikuro adapters and the runtime"
source: "https://nisoku.org/Saikuro/getting-started/installation/"
path: /getting-started/installation/
updated: 2026-07-04
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-07-04T10:28:43.641Z"
---
---
title: "Installation"
description: "Install Saikuro adapters and the runtime"
---

## Runtime

The Saikuro runtime is a Rust binary. Download a prebuilt binary from the [releases page](https://github.com/Nisoku/Saikuro/releases) or build from source:

```bash
git clone https://github.com/Nisoku/Saikuro.git
cd Saikuro
cargo build --release -p saikuro-runtime
```

For WASM targets, the runtime compiles to a WebAssembly module:

```bash
just wasm-rust-runtime
```

## Language Adapters

::: tabs

== tab "TypeScript"

```bash
npm install @nisoku/saikuro
```

```bash
pnpm add @nisoku/saikuro
```

```bash
yarn add @nisoku/saikuro
```

Works in Node.js 18+, browsers, and Bun.

== tab "Python"

```bash
pip install saikuro
```

Requires Python 3.11+.

== tab "C#"

```bash
dotnet add package Saikuro
```

Requires .NET 8+. Supports Blazor WASM via broadcast channel transport.

== tab "Rust"

```toml
[dependencies]
saikuro = "0.1"
```

Feature flags:

```toml
saikuro = { version = "0.1", features = ["storage", "fs-storage"] }
```

== tab "C"

```c
#include "saikuro.h"
// link libsaikuro_c.a
```

The C API expects a MessagePack-compatible buffer. See the [C adapter docs](../adapters/c/) for the full API.

== tab "C++"

```cpp
#include "saikuro/saikuro.hpp"
// link libsaikuro_cpp.a
```

Wraps the C API with RAII types. See the [C++ adapter docs](../adapters/cpp/).

:::

## WASM for the Browser

The TypeScript adapter works in the browser without any WASM compilation. For the runtime itself:

```bash
just wasm-all           # build all provider WASM modules
just wasm-rust-runtime  # build the WASM runtime (not included in wasm-all)
just web_demo dev       # build + start the dev server
```

See [WASM Guide](../guide/wasm) for browser integration details.

## Verify Installation

::: tabs

== tab "TypeScript"

```typescript
import { SaikuroClient, InMemoryTransport } from "@nisoku/saikuro";

// InMemoryTransport lets you test without a runtime
const [p, c] = InMemoryTransport.pair();
console.log("Saikuro types loaded");
```

== tab "Python"

```python
from saikuro import SaikuroClient, InMemoryTransport

p, c = InMemoryTransport.pair()
print("Saikuro types loaded")
```

:::

## Next Steps

::: grids
::: grid
::: button "Quick Start" ./quickstart.md icon:play
:::
::: grid
::: button "Core Concepts" ./concepts.md icon:book
:::
::: grid
::: button "WASM Guide" ../guide/wasm.md icon:globe
:::
:::
