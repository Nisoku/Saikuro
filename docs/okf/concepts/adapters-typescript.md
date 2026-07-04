---
type: concept
title: "TypeScript Adapter"
description: "Saikuro adapter for TypeScript and JavaScript"
source: "https://nisoku.org/Saikuro/adapters/typescript/"
path: /adapters/typescript/
updated: 2026-07-04
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-07-04T10:35:33.804Z"
---
---
title: "TypeScript Adapter"
description: "Saikuro adapter for TypeScript and JavaScript"
---

The TypeScript adapter works in Node.js 18+, browsers, and Bun. It supports all six invocation primitives and every transport type.

## Installation

```bash
npm install @nisoku/saikuro
```

## Client API

```typescript
import { SaikuroClient } from "@nisoku/saikuro";

// Connect via address string
const client = await SaikuroClient.connect("unix:///tmp/saikuro.sock");

// Or from an existing transport
const client = SaikuroClient.fromTransport(transport);
await client.open();

// Call a function
const result = await client.call("math.add", [1, 2]);

// Fire-and-forget
await client.cast("log.write", [{ level: "info", message: "started" }]);

// Stream
const stream = await client.stream("events.subscribe", []);
for await (const event of stream) {
  console.log(event);
}

// Channel
const chan = await client.channel("chat.session", [{ room: "general" }]);
await chan.send({ text: "hello" });
for await (const msg of chan) {
  console.log(msg);
}

// Batch
const [sum, product] = await client.batch([
  { target: "math.add", args: [1, 2] },
  { target: "math.multiply", args: [3, 4] },
]);

// Resource
const handle = await client.resource("files.open", ["/data.csv"]);

// Log
await client.log("info", "myapp", "started", { version: "1.0" });

// Close
await client.close();
```

### ClientOptions

```typescript
interface ClientOptions {
  defaultTimeoutMs?: number; // 0 = no timeout (default)
}
```

## Provider API

```typescript
import { SaikuroProvider, t } from "@nisoku/saikuro";

const provider = new SaikuroProvider("math");

// Register a sync function
provider.register("add", (a: number, b: number) => a + b);

// Register with schema
provider.register("add", (a: number, b: number) => a + b, {
  args: [
    { name: "a", type: t.i32() },
    { name: "b", type: t.i32() },
  ],
  returns: t.i32(),
  doc: "Add two integers.",
  capabilities: [],
  idempotent: true,
});

// Register a stream handler (async generator)
provider.register("events", async function* (filter) {
  while (true) {
    yield await poll(filter);
  }
});

// Decorator pattern
class MyService {
  @provider.decorator("greet")
  async greet(name: string) {
    return `Hello, ${name}`;
  }
}

// Serve
await provider.serve("unix:///tmp/saikuro.sock");
// Or serve on an existing transport
await provider.serveOn(transport);
```

### Type Descriptor Builders

The `t` object provides type-safe builders:

- `t.bool()`, `t.i32()`, `t.i64()`, `t.f32()`, `t.f64()`, `t.string()`, `t.bytes()`, `t.any()`, `t.unit()`
- `t.list(item)`, `t.map(key, value)`, `t.optional(inner)`
- `t.named(name)`, `t.stream(item)`, `t.channel(send, recv)`

## Transport

```typescript
import { makeTransport, InMemoryTransport } from "@nisoku/saikuro";

// Address-based
const transport = makeTransport("unix:///tmp/saikuro.sock");

// InMemory pair for testing
const [pt, ct] = InMemoryTransport.pair();
```

See [Transports](../../guide/transports) for the full address format reference.

## Export Surface

```typescript
// Core
SaikuroClient, SaikuroProvider, SaikuroStream, SaikuroChannel

// Transports
InMemoryTransport, WebSocketTransport, NodeStreamTransport,
WasmHostTransport, WasmHostConnector, WasmHostListener, makeTransport

// Errors
SaikuroError, FunctionNotFoundError, InvalidArgumentsError,
CapabilityDeniedError, TransportError, SaikuroTimeoutError,
ProviderError, NoProviderError, ProviderUnavailableError,
ProtocolVersionError, MalformedEnvelopeError, MessageTooLargeError,
BufferOverflowError, StreamClosedError, ChannelClosedError, OutOfOrderError

// Types
Handler, StreamHandler, AnyHandler, FunctionSchema, ArgDescriptor,
TypeDescriptor, ClientOptions, Transport, Envelope, ResponseEnvelope,
ErrorPayload, ErrorCode, InvocationType, StreamControl, SaikuroSchema,
ResourceHandle

// Schema extraction
// Import from "@nisoku/saikuro/schema-extractor"

// Logging
getLogger, setLogSink, setLogLevel, resetLogSink, createTransportSink
```
