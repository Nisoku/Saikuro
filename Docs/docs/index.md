---
title: "Saikuro"
description: "Cross-language invocation fabric for seamless multi-runtime integration"
---

**Saikuro** is a cross-language invocation fabric. It gives you typed function calls, streams, and channels across TypeScript, Python, C#, Rust, and whatever else you're running, all over a single transport-agnostic protocol with no hand-written bindings required.

::: callout tip
Saikuro means "cycle" in Japanese. The idea: your call leaves one language, travels through the runtime, and arrives in another, and the whole cycle is invisible to you as the developer.
:::

## What it solves

You're building something that needs two (or five) languages talking to each other. Normally that means a pile of HTTP boilerplate, some JSON you hope stays in sync, and a week of debugging silent failures when a buffer goes missing or a type doesn't round-trip cleanly.

Saikuro replaces all of that with a single protocol and a set of thin adapters. You define your functions once, Saikuro validates the schema, and callers in any supported language just... call them.

## Features

::: card Transport-Agnostic Protocol
Same wire format whether you're talking in-process, over a Unix socket, or across a WebSocket. Pick the transport that fits; the protocol doesn't care.
:::

::: card Strict Schema Model
Functions, types, capabilities, and namespaces are all declared in a schema the runtime enforces. No more "I thought it accepted a string" surprises.
:::

::: card Six Invocation Primitives
Call, cast, stream, channel, batch, and resource. Use the right primitive for the job instead of shoehorning everything into request/response.
:::

::: card Language Adapters
Thin clients for TypeScript, Python, C#, and Rust. They handle serialization and expose a consistent API. The runtime handles the rest.
:::

::: card Dev-Mode Discovery
In development, providers announce their schema automatically. No codegen step until you're ready to freeze things for production.
:::

## Quick Example

**TypeScript provider:**

```typescript
import { Provider } from "@nisoku/saikuro";

const provider = new Provider({ namespace: "math" });

provider.register("add", (a: number, b: number) => a + b);

await provider.serve();
```

**Python caller:**

```python
from saikuro import Client

client = Client()
await client.connect()

result = await client.call('math.add', [1, 2])
print(result)  # 3
```

That's it. No IDL file, no stub generator, no HTTP server.

## Installation

::: tabs
== tab "TypeScript"

```bash
npm install @nisoku/saikuro
```

== tab "Python"

```bash
pip install @nisoku/saikuro
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

- [Quick Start](./getting-started/quickstart): Get two languages talking in under 10 minutes
- [Core Concepts](./getting-started/concepts): Understand the runtime, adapters, and protocol
- [Invocation Primitives](./guide/invocations): Pick the right primitive for your use case
- [Protocol Reference](./api/): The full envelope spec
