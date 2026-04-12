---
title: "Transports"
description: "In-memory, Unix sockets, TCP, and WebSocket transport options"
---

The transport is the connection between an adapter and the Saikuro runtime. Saikuro picks one automatically based on where things are running, but you can override it when you need to.

## Automatic Selection

Saikuro selects the most efficient transport by default:

| Situation | Transport |
| --------- | --------- |
| Provider and runtime in the same process | In-memory |
| Same machine, different processes | Unix socket (Linux/macOS) or named pipe (Windows) |
| Different machines | TCP or WebSocket |

You don't have to configure anything for this to work. Start the runtime, start your providers and callers, and Saikuro figures out the right transport.

## In-Memory Transport

For same-process usage: testing, embedded runtimes, and WASM environments where sockets aren't available.

```typescript
// TypeScript
import { Provider, Client, InMemoryTransport } from 'saikuro';

const [providerTransport, clientTransport] = InMemoryTransport.pair();

const provider = new Provider({ namespace: 'math', transport: providerTransport });
provider.register('add', (a: number, b: number) => a + b);
await provider.serve();

const client = new Client({ transport: clientTransport });
await client.connect();

const result = await client.call('math.add', [1, 2]);
// result === 3
```

```python
# Python
from saikuro import Provider, Client, InMemoryTransport

provider_transport, client_transport = InMemoryTransport.pair()

provider = Provider(namespace='math', transport=provider_transport)

@provider.register('add')
def add(a: int, b: int) -> int:
    return a + b

await provider.serve()

client = Client(transport=client_transport)
await client.connect()

result = await client.call('math.add', [1, 2])
```

```csharp
// C#
var (providerTransport, clientTransport) = InMemoryTransport.Pair();

var provider = new Provider("math", providerTransport);
provider.Register<int, int, int>("add", (a, b) => a + b);
await provider.ServeAsync();

var client = new Client(clientTransport);
await client.ConnectAsync();

var result = await client.CallAsync<int>("math.add", new object[] { 1, 2 });
```

In-memory transport is the default for tests. It gives you the full Saikuro behavior with no network overhead.

::: callout info
In-memory transport is the only transport available in WASM environments. The C# adapter has `#if WASM` guards that limit the available transports accordingly.
:::

## Unix Socket / Named Pipe

For same-machine cross-process communication. This is the default when the runtime and adapter are in different processes on the same host.

The runtime binds to a well-known socket path. Adapters connect to it automatically:

```typescript
// TypeScript, explicit socket path
const client = new Client({
  transport: 'unix',
  socketPath: '/tmp/saikuro.sock'
});
```

```python
# Python, explicit socket path
client = Client(transport='unix', socket_path='/tmp/saikuro.sock')
```

On Windows, named pipes are used instead:

```csharp
// C#, named pipe (Windows)
var client = new Client(new NamedPipeTransport("saikuro"));
```

## WebSocket Transport

For browser clients or any situation where you're crossing a network boundary and need WebSocket-compatible transport.

```typescript
// TypeScript, browser or Node.js WebSocket client
const client = new Client({
  transport: 'websocket',
  url: 'ws://localhost:7700'
});

await client.connect();
const result = await client.call('math.add', [1, 2]);
```

The runtime exposes a WebSocket endpoint. Configure it at startup:

```bash
saikuro-runtime --ws-port 7700
```

WebSocket transport works in browsers natively. The TypeScript adapter is plain JavaScript/TypeScript with no WASM compilation needed.

## TCP Transport

For direct TCP connections between machines. Lower overhead than WebSocket when you don't need HTTP compatibility.

```typescript
// TypeScript
const client = new Client({
  transport: 'tcp',
  host: '10.0.0.5',
  port: 7700
});
```

```python
# Python
client = Client(transport='tcp', host='10.0.0.5', port=7700)
```

Start the runtime with TCP enabled:

```bash
saikuro-runtime --tcp-port 7700
```

## Transport Requirements

All transports must satisfy these properties for Saikuro to work correctly:

- **Ordered delivery**: Messages arrive in the order they were sent
- **Backpressure**: Senders slow down when the receiver falls behind
- **Binary-safe**: The transport carries raw bytes without modification

The in-memory, Unix socket, and TCP transports satisfy all three by default. WebSocket does too once framing is handled.

## TLS

For TCP and WebSocket transports, you can enable TLS:

```bash
saikuro-runtime --tcp-port 7700 --tls-cert ./cert.pem --tls-key ./key.pem
```

The adapters detect TLS from the URL scheme (`wss://`) or an explicit flag:

```typescript
// TypeScript
const client = new Client({
  transport: 'websocket',
  url: 'wss://example.com/saikuro'
});
```

```python
# Python
client = Client(transport='tcp', host='example.com', port=7700, tls=True)
```

## Testing with In-Memory Transport

For unit and integration tests, use `InMemoryTransport.pair()` to wire up a provider and client directly without starting a runtime process. This is the recommended pattern for tests:

```typescript
// TypeScript test
import { describe, it, expect } from 'vitest';
import { Provider, Client, InMemoryTransport } from 'saikuro';

describe('math provider', () => {
  it('adds two numbers', async () => {
    const [pt, ct] = InMemoryTransport.pair();

    const provider = new Provider({ namespace: 'math', transport: pt });
    provider.register('add', (a: number, b: number) => a + b);
    await provider.serve();

    const client = new Client({ transport: ct });
    await client.connect();

    expect(await client.call('math.add', [1, 2])).toBe(3);
  });
});
```

```python
# Python test
import pytest
from saikuro import Provider, Client, InMemoryTransport

@pytest.mark.asyncio
async def test_add():
    provider_t, client_t = InMemoryTransport.pair()

    provider = Provider(namespace='math', transport=provider_t)

    @provider.register('add')
    def add(a: int, b: int) -> int:
        return a + b

    await provider.serve()

    client = Client(transport=client_t)
    await client.connect()

    assert await client.call('math.add', [1, 2]) == 3
```

## Next Steps

- [Language Adapters](../adapters/): Adapter-specific transport options and configuration
- [Protocol Reference](../api/): How the wire format works at the byte level
- [Examples](./examples): Real patterns with different transport configurations
