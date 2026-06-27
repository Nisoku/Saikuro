---
type: concept
title: "TypeScript Examples"
description: "TypeScript adapter usage patterns"
source: "https://nisoku.org/Saikuro/adapters/typescript/examples/"
path: /adapters/typescript/examples/
updated: 2026-06-27
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-06-27T13:11:26.475Z"
---
---
title: "TypeScript Examples"
description: "TypeScript adapter usage patterns"
---

## Provider with Schema

```typescript
import { SaikuroProvider, t } from "@nisoku/saikuro";

const provider = new SaikuroProvider("math");

provider.register("add", (a: number, b: number) => a + b, {
  args: [{ name: "a", type: t.i32() }, { name: "b", type: t.i32() }],
  returns: t.i32(),
  doc: "Add two integers.",
});

provider.register("divide", (a: number, b: number) => {
  if (b === 0) throw new Error("division by zero");
  return a / b;
}, {
  args: [{ name: "a", type: t.f64() }, { name: "b", type: t.f64() }],
  returns: t.f64(),
  capabilities: ["math.divide"],
});

await provider.serve("unix:///tmp/saikuro.sock");
```

## Stream Provider

```typescript
provider.register("events.subscribe", async function* (topic: string) {
  const sub = await subscribeToTopic(topic);
  try {
    for await (const event of sub) {
      yield event;
    }
  } finally {
    await sub.unsubscribe();
  }
});
```

## Client with Timeout

```typescript
const client = await SaikuroClient.connect("tcp://10.0.0.5:7700", {
  defaultTimeoutMs: 5000,
});

try {
  const result = await client.call("math.add", [1, 2], { timeoutMs: 1000 });
} catch (err) {
  if (err instanceof SaikuroTimeoutError) {
    console.error("call timed out");
  }
}
```

## Testing with InMemory

```typescript
import { describe, it, expect } from "vitest";
import { SaikuroProvider, SaikuroClient, InMemoryTransport } from "@nisoku/saikuro";

describe("math", () => {
  it("adds", async () => {
    const [pt, ct] = InMemoryTransport.pair();
    const provider = new SaikuroProvider("math");
    provider.register("add", (a: number, b: number) => a + b);
    await provider.serveOn(pt);

    const client = await SaikuroClient.openOn(ct);
    expect(await client.call("math.add", [1, 2])).toBe(3);
  });
});
```
