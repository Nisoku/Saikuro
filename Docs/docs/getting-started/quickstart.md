---
title: "Quick Start"
description: "Get two languages talking with Saikuro in under 10 minutes"
---

This guide gets you from zero to a working cross-language call in about 10 minutes. We'll write a simple math provider in TypeScript and call it from Python.

## Before You Start

You need:

- Node.js 18+ and npm
- Python 3.11+
- A Saikuro runtime running locally (or use in-memory transport for same-process demos)

## Step 1: Install the adapters

::: tabs
== tab "TypeScript"

```bash
npm install saikuro
```

== tab "Python"

```bash
pip install saikuro
```

:::

## Step 2: Write a provider

Providers register functions under a namespace. Here's a TypeScript math provider:

```typescript
// provider.ts
import { SaikuroProvider } from "@nisoku/saikuro";

const provider = new SaikuroProvider("math");

provider.register("add", (a: number, b: number): number => {
  return a + b;
});

provider.register("multiply", (a: number, b: number): number => {
  return a * b;
});

await provider.serve("tcp://127.0.0.1:7700");
console.log("math provider ready");
```

Run it:

```bash
npx tsx provider.ts
```

## Step 3: Call it from Python

```python
# caller.py
import asyncio
from saikuro import SaikuroClient

async def main():
    client = await SaikuroClient.connect('tcp://127.0.0.1:7700')

    result = await client.call('math.add', [10, 32])
    print(f'10 + 32 = {result}')  # 10 + 32 = 42

    result = await client.call('math.multiply', [6, 7])
    print(f'6 * 7 = {result}')  # 6 * 7 = 42

asyncio.run(main())
```

Run it:

```bash
python caller.py
```

You just called a TypeScript function from Python. No HTTP server, no serialization glue, no shared interface file.

## Step 4: Try the other direction

Providers don't have to be TypeScript. Here's the same provider in Python:

```python
# provider.py
import asyncio
from saikuro import SaikuroProvider

async def main():
    provider = SaikuroProvider('math')

    @provider.register('add')
    def add(a: int, b: int) -> int:
        return a + b

    @provider.register('multiply')
    def multiply(a: int, b: int) -> int:
        return a * b

    await provider.serve('tcp://127.0.0.1:7700')
    print('math provider ready')

asyncio.run(main())
```

And call it from TypeScript:

```typescript
// caller.ts
import { SaikuroClient } from "@nisoku/saikuro";

const client = await SaikuroClient.connect("tcp://127.0.0.1:7700");

const result = await client.call("math.add", [10, 32]);
console.log(`10 + 32 = ${result}`); // 10 + 32 = 42
```

Same call, same result. The caller doesn't know or care which language is on the other end.

## What just happened

When your provider called `serve()`, it:

1. Connected to the Saikuro runtime
2. Announced its namespace and function signatures
3. Started handling incoming invocations

When your caller called `client.call('math.add', [10, 32])`:

1. The adapter serialized the arguments to MessagePack
2. Sent an envelope to the runtime
3. The runtime validated and routed it to the `math` provider
4. The provider deserialized, ran your function, and sent back the result
5. The adapter returned the deserialized result to your code

All of that is invisible to you. From your code's perspective, you just called a function.

## Next Steps

- [Core Concepts](./concepts): Understand providers, namespaces, and the runtime
- [Invocation Primitives](../guide/invocations): Beyond request/response: streams, channels, and more
- [Language Adapters](../guide/adapters): C# and Rust adapter usage
- [Examples](../guide/examples): Real patterns for real projects
