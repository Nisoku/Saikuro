---
type: concept
title: Transports
description: "In-memory, Unix sockets, TCP, WebSocket, and WasmHost transport options"
source: "https://nisoku.org/Saikuro/guide/transports/"
path: /guide/transports/
updated: 2026-07-09
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-07-09T20:42:46.342Z"
---
---
title: "Transports"
description: "In-memory, Unix sockets, TCP, WebSocket, and WasmHost transport options"
---

The transport connects adapters to the Saikuro runtime. All transports carry the same MessagePack-encoded envelopes; only the underlying mechanism differs.

## Address Format

All adapters use a unified address string to select and configure transports:

| Format                                 | Transport        | Platform       |
|----------------------------------------|------------------|----------------|
| `memory://` (or `memory://name`)       | InMemory         | All (testing)  |
| `unix:///path/to/socket`               | Unix socket      | Linux, macOS   |
| `tcp://host:port`                      | TCP              | All native     |
| `ws://host/path`                       | WebSocket        | All            |
| `wss://host/path`                      | WebSocket TLS    | All            |
| `wasm-host://channel` (or `wasm-host`) | BroadcastChannel | WASM (browser) |

The factory functions (`makeTransport` / `make_transport`) parse these addresses automatically:

```typescript
// TypeScript - factory picks the right transport
import { makeTransport } from "@nisoku/saikuro";

const transport = makeTransport("unix:///tmp/saikuro.sock");
```

```python
# Python
from saikuro import make_transport

transport = make_transport("unix:///tmp/saikuro.sock")
```

```csharp
// C# - TransportFactory parses address strings
var transport = TransportFactory.Create("unix:///tmp/saikuro.sock");
```

## InMemory Transport

For same-process usage: testing, embedded runtimes, and WASM environments.

Both sides of the transport are created as a pair:

```typescript
import { InMemoryTransport } from "@nisoku/saikuro";

const [providerTransport, clientTransport] = InMemoryTransport.pair();

const provider = new SaikuroProvider("math");
// provider.serveOn(providerTransport); ...

const client = await SaikuroClient.openOn(clientTransport);
```

```python
from saikuro import InMemoryTransport

provider_transport, client_transport = InMemoryTransport.pair()
```

InMemory transport is the default for tests. It gives you full Saikuro behavior with zero network overhead.

## Unix Socket

For same-machine cross-process communication.

```typescript
const client = await SaikuroClient.connect("unix:///tmp/saikuro.sock");
```

```python
async with SaikuroClient.connect("unix:///tmp/saikuro.sock") as client:
    ...
```

```rust
let client = Client::connect("unix:///tmp/saikuro.sock").await?;
```

## TCP Transport

For cross-machine communication.

```typescript
const client = await SaikuroClient.connect("tcp://10.0.0.5:7700");
```

```python
async with SaikuroClient.connect("tcp://10.0.0.5:7700") as client:
    ...
```

## WebSocket Transport

For browser clients or when you need HTTP compatibility.

```typescript
// Browser or Node.js
const client = await SaikuroClient.connect("ws://localhost:7700");
```

```python
async with SaikuroClient.connect("ws://localhost:7700") as client:
    ...
```

TLS is supported via the `wss://` scheme:

```typescript
const client = await SaikuroClient.connect("wss://example.com/saikuro");
```

## WasmHost Transport

For same-origin browser WASM communication using the BroadcastChannel API. Both the runtime and adapters run in the browser, and the transport bridges them via message passing.

```typescript
// Browser WASM context
import { WasmHostTransport } from "@nisoku/saikuro";

const transport = new WasmHostTransport("saikuro");
```

The address string form is `wasm-host://channel-name`:

```typescript
const client = await SaikuroClient.connect("wasm-host://saikuro");
```

See the [WASM Guide](./wasm) for full details on WASM integration.

## In C and C++

Transport configuration is passed via the runtime handle at initialization:

```c
SaikuroRuntimeConfig config = {
    .address = "unix:///tmp/saikuro.sock",
    .transport_kind = SAIKURO_TRANSPORT_UNIX
};
SaikuroHandle* handle = saikuro_init(&config);
```

## Transport Requirements

All transports must provide:

- **Ordered delivery**: messages arrive in the order sent
- **Backpressure**: senders slow down when the receiver falls behind
- **Binary-safe**: raw bytes without modification

InMemory, Unix, TCP, and WebSocket all satisfy these by default.

## Testing with InMemory Transport

```typescript
import { describe, it, expect } from "vitest";
import { SaikuroProvider, SaikuroClient, InMemoryTransport } from "@nisoku/saikuro";

describe("math provider", () => {
  it("adds two numbers", async () => {
    const [pt, ct] = InMemoryTransport.pair();

    const provider = new SaikuroProvider("math");
    provider.register("add", (a: number, b: number) => a + b);
    await provider.serveOn(pt);

    const client = await SaikuroClient.openOn(ct);
    expect(await client.call("math.add", [1, 2])).toBe(3);
  });
});
```

```python
import pytest
from saikuro import SaikuroProvider, SaikuroClient, InMemoryTransport

@pytest.mark.asyncio
async def test_add():
    pt, ct = InMemoryTransport.pair()
    provider = SaikuroProvider("math")

    @provider.register("add")
    def add(a: int, b: int) -> int:
        return a + b

    await provider.serve_on(pt)
    async with SaikuroClient.open_on(ct) as client:
        assert await client.call("math.add", [1, 2]) == 3
```

## Next Steps

::: grids
::: grid
::: button "Storage" ./storage.md icon:database
:::
::: grid
::: button "WASM Guide" ./wasm.md icon:globe
:::
::: grid
::: button "Language Adapters" ../adapters/ icon:code
:::
::: grid
::: button "Protocol Reference" ../api/ icon:box
:::
:::
