---
title: "Language Adapters"
description: "TypeScript, Python, C#, and Rust adapter reference"
---

Adapters are the language-specific layer between your code and the Saikuro runtime. Each adapter handles serialization and exposes a consistent `Provider` / `Client` API. They're deliberately thin: no routing, no schema validation, no capability enforcement. That's all in the runtime.

## TypeScript / JavaScript

The TypeScript adapter runs natively in Node.js and browsers (plain JavaScript, no WASM). Install it:

```bash
npm install saikuro
```

### Provider

```typescript
import { Provider } from "@nisoku/saikuro";

const provider = new Provider({ namespace: "math" });

// Register a function
provider.register("add", (a: number, b: number): number => {
  return a + b;
});

// Register an async function
provider.register("fetchUser", async (id: string) => {
  return await db.users.findById(id);
});

// Register a stream
provider.registerStream("events", async function* (filter: string) {
  for await (const event of eventSource(filter)) {
    yield event;
  }
});

// Register a channel handler
provider.registerChannel("chat", async (args, chan) => {
  for await (const msg of chan.incoming()) {
    await chan.send({ echo: msg });
  }
});

await provider.serve();
```

### Client

```typescript
import { Client } from "@nisoku/saikuro";

const client = new Client();
await client.connect();

// Call
const sum = await client.call("math.add", [1, 2]);

// Cast (fire and forget)
await client.cast("log.write", [{ message: "hello" }]);

// Stream
for await (const event of client.stream("events.subscribe", ["errors"])) {
  console.log(event);
}

// Channel
const chan = await client.channel("chat.open", []);
await chan.send({ text: "hello" });
for await (const msg of chan) {
  console.log(msg);
}
await chan.close();

// Batch
const [a, b] = await client.batch([
  { target: "math.add", args: [1, 2] },
  { target: "math.multiply", args: [3, 4] },
]);
```

### Dev Mode

In dev mode, the provider extracts its schema automatically using the TypeScript compiler API and announces it to the runtime:

```typescript
const provider = new Provider({
  namespace: "math",
  dev: true, // enable dev mode
  sourceFiles: ["./src/math-provider.ts"], // where to extract types from
});
```

When `dev: true` is set, `serve()` announces the schema before accepting calls.

### Transport Configuration

```typescript
// Explicit WebSocket transport (browser)
const client = new Client({
  transport: "websocket",
  url: "ws://localhost:7700",
});

// Explicit Unix socket
const client = new Client({
  transport: "unix",
  socketPath: "/tmp/saikuro.sock",
});

// In-memory (for testing)
import { InMemoryTransport } from "@nisoku/saikuro";
const [pt, ct] = InMemoryTransport.pair();
const provider = new Provider({ namespace: "math", transport: pt });
const client = new Client({ transport: ct });
```

---

## Python

Python 3.11+. Install:

```bash
pip install saikuro
```

### Provider

```python
from saikuro import Provider

provider = Provider(namespace='math')

# Decorator-based registration
@provider.register('add')
def add(a: int, b: int) -> int:
    return a + b

# Async functions work too
@provider.register('fetch_user')
async def fetch_user(user_id: str):
    return await db.users.find(user_id)

# Stream
@provider.register_stream('events')
async def events(filter: str):
    async for event in event_source(filter):
        yield event

# Channel
@provider.register_channel('chat')
async def chat(args, chan):
    async for msg in chan.incoming():
        await chan.send({'echo': msg})

await provider.serve()
```

### Client

```python
from saikuro import Client

client = Client()
await client.connect()

# Call
result = await client.call('math.add', [1, 2])

# Cast
await client.cast('log.write', [{'message': 'hello'}])

# Stream
async for event in client.stream('events.subscribe', ['errors']):
    print(event)

# Channel
async with client.channel('chat.open', []) as chan:
    await chan.send({'text': 'hello'})
    async for msg in chan:
        print(msg)

# Batch
results = await client.batch([
    {'target': 'math.add', 'args': [1, 2]},
    {'target': 'math.multiply', 'args': [3, 4]},
])
```

### Transport Configuration

```python
# WebSocket
client = Client(transport='websocket', url='ws://localhost:7700')

# Unix socket
client = Client(transport='unix', socket_path='/tmp/saikuro.sock')

# In-memory (for testing)
from saikuro import InMemoryTransport
provider_t, client_t = InMemoryTransport.pair()
provider = Provider(namespace='math', transport=provider_t)
client = Client(transport=client_t)
```

---

## C#

.NET 8+. Install:

```bash
dotnet add package Saikuro
```

### Provider

```csharp
using Saikuro;

var provider = new Provider("math");

// Register a synchronous function
provider.Register<int, int, int>("add", (a, b) => a + b);

// Register an async function
provider.Register<string, Task<User>>("fetchUser", async (id) =>
{
    return await db.Users.FindByIdAsync(id);
});

// Register a stream
provider.RegisterStream<string, IAsyncEnumerable<Event>>("events", async (filter) =>
{
    await foreach (var evt in EventSource(filter))
        yield return evt;
});

// Register a channel handler
provider.RegisterChannel("chat", async (args, chan) =>
{
    await foreach (var msg in chan.IncomingAsync())
    {
        await chan.SendAsync(new { echo = msg });
    }
});

await provider.ServeAsync();
```

### Client

```csharp
using Saikuro;

var client = new Client();
await client.ConnectAsync();

// Call
var sum = await client.CallAsync<int>("math.add", new object[] { 1, 2 });

// Cast
await client.CastAsync("log.write", new object[] { new { message = "hello" } });

// Stream
await foreach (var evt in client.StreamAsync<Event>("events.subscribe", new[] { "errors" }))
{
    Console.WriteLine(evt);
}

// Channel
var chan = await client.ChannelAsync("chat.open", Array.Empty<object>());
await chan.SendAsync(new { text = "hello" });
await foreach (var msg in chan.ReceiveAsync())
{
    Console.WriteLine(msg);
}
await chan.CloseAsync();

// Batch
var results = await client.BatchAsync(new[]
{
    new BatchItem("math.add", new object[] { 1, 2 }),
    new BatchItem("math.multiply", new object[] { 3, 4 }),
});
```

### Transport Configuration

```csharp
// WebSocket (works in Blazor WASM too)
var client = new Client(new WebSocketTransport("ws://localhost:7700"));

// Unix socket
var client = new Client(new UnixSocketTransport("/tmp/saikuro.sock"));

// In-memory (for testing)
var (providerTransport, clientTransport) = InMemoryTransport.Pair();
var provider = new Provider("math", providerTransport);
var client = new Client(clientTransport);
```

::: callout info
In WASM (Blazor), only InMemory and WebSocket transports are available. The `#if WASM` build flag enables the right subset automatically when you target a WASM project.
:::

---

## Rust

Add the crate:

```toml
[dependencies]
saikuro = "0.1"
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

### Provider

Handlers receive a `Vec<serde_json::Value>` and return `Result<serde_json::Value>`:

```rust
use saikuro::{Provider, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let mut provider = Provider::new("math");

    provider.register("add", |args: Vec<serde_json::Value>| async move {
        let a = args[0].as_i64().unwrap_or(0);
        let b = args[1].as_i64().unwrap_or(0);
        Ok(serde_json::json!(a + b))
    });

    provider.serve("tcp://127.0.0.1:7700").await
}
```

`serve` blocks until the runtime closes the connection. Pass any supported address scheme: `tcp://`, `ws://`, or `unix://`.

### Client

```rust
use saikuro::{Client, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::connect("tcp://127.0.0.1:7700").await?;

    let result = client.call("math.add", vec![
        serde_json::json!(1),
        serde_json::json!(2),
    ]).await?;

    println!("{result}"); // 3

    client.close().await?;
    Ok(())
}
```

### Cast and Batch

```rust
// Fire and forget
client.cast("log.write", vec![serde_json::json!({"message": "hello"})]).await?;

// Multiple calls in one round-trip
let results = client.batch(vec![
    ("math.add".into(),      vec![serde_json::json!(1), serde_json::json!(2)]),
    ("math.add".into(),      vec![serde_json::json!(3), serde_json::json!(4)]),
]).await?;
```

### Streaming

```rust
let mut stream = client.stream("events.subscribe", vec![
    serde_json::json!("errors"),
]).await?;

while let Some(item) = stream.next().await {
    println!("{}", item?);
}
```

### Schema Metadata

Register with a schema to enable codegen and runtime validation:

```rust
use saikuro::{Provider, RegisterOptions, FunctionSchema, Result};

let mut provider = Provider::new("math");

provider.register_with_options(
    "add",
    |args: Vec<serde_json::Value>| async move {
        let a = args[0].as_i64().unwrap_or(0);
        let b = args[1].as_i64().unwrap_or(0);
        Ok(serde_json::json!(a + b))
    },
    RegisterOptions {
        schema: Some(FunctionSchema {
            doc: Some("Add two integers.".into()),
            idempotent: true,
            ..Default::default()
        }),
    },
);
```

### Transport Configuration

By default, TCP is enabled. Enable additional transports with Cargo features:

```toml
[dependencies]
saikuro = { version = "0.1", features = ["tcp", "ws", "unix"] }
```

| Feature         | Transport                             |
| --------------- | ------------------------------------- |
| `tcp` (default) | `tcp://host:port`                     |
| `ws`            | `ws://host:port` or `wss://host:port` |
| `unix`          | `unix:///path/to/socket` (Unix only)  |

---

## Error Handling

All adapters use the same error model. Errors come back as a structured object with a `code`, `message`, and optional `details`:

```typescript
// TypeScript
try {
  await client.call("admin.delete_everything", []);
} catch (err) {
  if (err.code === "CapabilityDenied") {
    console.error("Not authorized:", err.message);
  }
}
```

```python
# Python
from saikuro import SaikuroError

try:
    await client.call('admin.delete_everything', [])
except SaikuroError as e:
    if e.code == 'CapabilityDenied':
        print(f'Not authorized: {e.message}')
```

```csharp
// C#
try
{
    await client.CallAsync<object>("admin.DeleteEverything", Array.Empty<object>());
}
catch (SaikuroException ex) when (ex.Code == "CapabilityDenied")
{
    Console.Error.WriteLine($"Not authorized: {ex.Message}");
}
```

See the [Protocol Reference](../api/) for the full list of error codes.

## Next Steps

- [Code Generation](./codegen): Generate typed client stubs from a frozen schema
- [Transports](./transports): In-memory, sockets, and WebSocket in depth
- [Examples](./examples): Real multi-language patterns
- [Protocol Reference](../api/): The wire format every adapter speaks
