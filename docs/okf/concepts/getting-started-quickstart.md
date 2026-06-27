---
type: concept
title: "Quick Start"
description: "Get two languages talking with Saikuro in under 10 minutes"
source: "https://nisoku.org/Saikuro/getting-started/quickstart/"
path: /getting-started/quickstart/
updated: 2026-06-27
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-06-27T14:04:05.296Z"
---
---
title: "Quick Start"
description: "Get two languages talking with Saikuro in under 10 minutes"
---

This guide gets you from zero to a working cross-language call. You will write a math provider in TypeScript and call it from Python.

::: callout tip "Prerequisites"
Node.js 18+, Python 3.11+, and the Saikuro runtime running locally (or use in-memory transport for same-process demos). See [Installation](./installation).
:::

::: steps

1. **Install adapters**

   ::: tabs

   == tab "TypeScript"

   ```bash
   npm install @nisoku/saikuro
   ```

   == tab "Python"

   ```bash
   pip install saikuro
   ```

   :::

2. **Write a TypeScript provider**

   ```typescript
   // provider.ts
   import { SaikuroProvider } from "@nisoku/saikuro";

   const provider = new SaikuroProvider("math");

   provider.register("add", (a: number, b: number) => a + b);
   provider.register("multiply", (a: number, b: number) => a * b);

   await provider.serve("unix:///tmp/saikuro.sock");
   ```

   Run it:

   ```bash
   npx tsx provider.ts
   ```

3. **Call it from Python**

   ```python
   # caller.py
   import asyncio
   from saikuro import SaikuroClient

   async def main():
       async with SaikuroClient.connect("unix:///tmp/saikuro.sock") as client:
           result = await client.call("math.add", [10, 32])
           print(f"10 + 32 = {result}")  # 10 + 32 = 42

           result = await client.call("math.multiply", [6, 7])
           print(f"6 * 7 = {result}")  # 6 * 7 = 42

   asyncio.run(main())
   ```

   ```bash
   python caller.py
   ```

   You just called TypeScript from Python. No HTTP server, no serialization glue, no shared interface file.

4. **Go the other direction**

   Same provider in Python:

   ```python
   # provider.py
   import asyncio
   from saikuro import SaikuroProvider

   provider = SaikuroProvider("math")

   @provider.register("add")
   def add(a: int, b: int) -> int:
       return a + b

   @provider.register("multiply")
   def multiply(a: int, b: int) -> int:
       return a * b

   asyncio.run(provider.serve("unix:///tmp/saikuro.sock"))
   ```

   Call it from TypeScript:

   ```typescript
   import { SaikuroClient } from "@nisoku/saikuro";

   const client = await SaikuroClient.connect("unix:///tmp/saikuro.sock");
   const result = await client.call("math.add", [10, 32]);
   console.log(`10 + 32 = ${result}`);
   ```

   The caller does not know or care which language is on the other end.

:::

## What just happened

When your provider called `serve()`, it:

1. Connected to the Saikuro runtime over the transport
2. Announced its namespace and function signatures
3. Started handling incoming invocations

When your caller ran `client.call("math.add", [10, 32])`:

1. The adapter serialized arguments to MessagePack
2. Sent an envelope to the runtime
3. The runtime validated and routed it to the `math` provider
4. The provider deserialized, ran your function, and sent back the result
5. The adapter returned the deserialized result to your code

All of that is invisible. From your code's perspective you just called a function.

## Next Steps

::: grids
::: grid
::: button "Installation" ./installation.md icon:download
:::
::: grid
::: button "Core Concepts" ./concepts.md icon:book
:::
::: grid
::: button "Invocation Primitives" ../guide/invocations.md icon:zap
:::
::: grid
::: button "Transports" ../guide/transports.md icon:radio
:::
:::
