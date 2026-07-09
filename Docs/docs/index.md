---
title: "Saikuro"
description: "Cross-language invocation fabric for seamless multi-runtime integration"
---

::: hero layout:split glow:true

# Saikuro

Cross-language invocation fabric. Typed function calls, streams, and channels across TypeScript, Python, C#, Rust, C, and C++.

::: tag "Transport Agnostic"
::: tag "Strict Schema"
::: tag "Six Primitives"
::: tag "WASM Ready"

::: button "Quick Start" ./getting-started/quickstart.md icon:play
::: button "GitHub" external:https://github.com/Nisoku/Saikuro icon:github

== side

::: card "Why Saikuro?"
Normal multi-language setups mean HTTP boilerplate, JSON that drifts out of sync, and a week debugging silent failures.

**Saikuro replaces all of it.** One protocol, thin adapters, zero hand-written bindings. Define functions once, call them from any language.
:::

:::

## Features

::: grids
::: grid
::: card "Transport Agnostic Protocol" icon:radio
Same wire format whether you are talking in-process, over a Unix socket, or across a WebSocket. Pick the transport that fits; the protocol does not care.
:::
:::

::: grid
::: card "Strict Schema Model" icon:file-text
Functions, types, capabilities, and namespaces are all declared in a schema the runtime enforces. No more "I thought it accepted a string" surprises.
:::
:::

::: grid
::: card "Six Invocation Primitives" icon:zap
Call, cast, stream, channel, batch, and resource. Use the right primitive for the job instead of shoehorning everything into request/response.
:::
:::

::: grid
::: card "Language Adapters" icon:code
Thin clients for TypeScript, Python, C#, Rust, C, and C++. Consistent API surface across every language. Adding a new language means writing a transport and a schema mapper.
:::
:::

::: grid
::: card "Dev Mode Discovery" icon:search
Providers announce their schema automatically during development. No codegen step until you freeze things for production.
:::
:::

::: grid
::: card "WASM Native" icon:globe
Run the runtime and providers entirely in the browser via WASM. WasmHost transport uses BroadcastChannel for same-origin adapter communication. Storage backends for IndexedDB, OPFS, and Web Storage.
:::
:::

:::

## Quick Example

**TypeScript provider, Python caller. No IDL, no stub generator, no HTTP server.**

::: tabs

== tab "TypeScript Provider"

```typescript
import { SaikuroProvider } from "@nisoku/saikuro";

const provider = new SaikuroProvider("math");
provider.register("add", (a: number, b: number) => a + b);
await provider.serve("unix:///tmp/saikuro.sock");
```

== tab "Python Caller"

```python
from saikuro import SaikuroClient

async with SaikuroClient.connect("unix:///tmp/saikuro.sock") as client:
    result = await client.call("math.add", [1, 2])
    print(result)  # 3
```

:::

## Installation

::: tabs

== tab "TypeScript"

```bash
npm install @nisoku/saikuro
```

== tab "Python"

```bash
pip install saikuro
```

== tab "C#"

```bash
dotnet add package Saikuro
```

== tab "Rust"

```toml
[dependencies]
saikuro = "0.1"
```

:::

## Next Steps

::: grids
::: grid
::: button "Quick Start" ./getting-started/quickstart.md icon:play
:::
::: grid
::: button "Installation" ./getting-started/installation.md icon:download
:::
::: grid
::: button "Core Concepts" ./getting-started/concepts.md icon:book
:::
::: grid
::: button "Protocol Reference" ./api/ icon:code
:::
:::
