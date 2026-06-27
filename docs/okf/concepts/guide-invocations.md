---
type: concept
title: "Invocation Primitives"
description: "The six ways to communicate across language boundaries with Saikuro"
source: "https://nisoku.org/Saikuro/guide/invocations/"
path: /guide/invocations/
updated: 2026-06-27
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-06-27T14:04:05.298Z"
---
---
title: "Invocation Primitives"
description: "The six ways to communicate across language boundaries with Saikuro"
---

Saikuro gives you six primitives. They cover the full range of cross-language communication patterns, so do not force everything into request/response when a stream or channel is the right tool.

## Call

Request/response. The caller sends arguments and waits for a single return value. The most common primitive.

```typescript
// TypeScript
const result = await client.call("math.add", [1, 2]);
```

```python
# Python
result = await client.call("math.add", [1, 2])
```

```csharp
// C#
var result = await client.CallAsync<int>("math.add", new object[] { 1, 2 });
```

```rust
// Rust
let result: i32 = client.call("math.add", &[1.into(), 2.into()]).await?;
```

Use `call` when you need a response and the response is a single value. Supports optional `capability` and `timeout` parameters.

## Cast

Fire and forget. The caller sends a message and does not wait for a response. Best-effort delivery.

```typescript
await client.cast("log.write", [{ level: "info", message: "started" }]);
```

```python
await client.cast("log.write", [{"level": "info", "message": "started"}])
```

Use `cast` for notifications, audit events, log writes, and anything where you do not need to know if it succeeded. Faster than `call` because no response is sent.

## Stream

One-way sequence of messages from provider to caller. The caller iterates over values as they arrive.

```typescript
for await (const event of client.stream("events.subscribe", [])) {
  console.log(event);
}
```

```python
async for event in client.stream("events.subscribe", []):
    print(event)
```

Provider side (TypeScript):

```typescript
// An async generator function becomes a stream provider
provider.register("events.subscribe", async function* (filter) {
  while (true) {
    const event = await waitForNextEvent(filter);
    yield event;
  }
});
```

Provider side (Python):

```python
@provider.register("events.subscribe")
async def subscribe(filter):
    while True:
        event = await wait_for_next_event(filter)
        yield event
```

Use `stream` for subscriptions, paginated results, log tailing, and progress reporting. Streams are ordered: messages arrive in the order the provider sent them.

## Channel

Bidirectional stream with backpressure. Both sides can send and receive. The runtime applies flow control so neither side overwhelms the other.

```typescript
const chan = await client.channel("chat.session", [{ room: "general" }]);

chan.send({ type: "message", text: "hello" });

for await (const msg of chan) {
  console.log(msg.text);
}

await chan.close();
```

```python
async with client.channel("chat.session", [{"room": "general"}]) as chan:
    await chan.send({"type": "message", "text": "hello"})
    async for msg in chan:
        print(msg["text"])
```

Provider side:

```typescript
provider.register("chat.session", async (args, chan) => {
  const { room } = args[0];
  for await (const msg of chan) {
    await broadcastToRoom(room, msg);
    await chan.send({ type: "ack", id: msg.id });
  }
});
```

Use `channel` for interactive sessions and bidirectional data transfer.

::: callout warning
Channels implement backpressure. If the consumer falls behind, `send()` blocks until capacity frees up.
:::

## Batch

Multiple calls in one envelope. Minimizes round trips when you have several independent calls.

```typescript
const [sum, product] = await client.batch([
  { target: "math.add", args: [1, 2] },
  { target: "math.multiply", args: [3, 4] },
]);
```

```python
results = await client.batch([
    ("math.add", [1, 2]),
    ("math.multiply", [3, 4]),
])
```

Batch calls can span namespaces. Each call is routed independently. Results come back in the same order as requests.

## Resource

An opaque handle to large or external data. Instead of sending a 50 MB file inline, send a resource reference and let the provider fetch it through the runtime.

```typescript
const handle = await client.resource("files.open", ["/data/report.csv"]);
// handle.id, handle.mime_type, handle.size, handle.uri
```

```python
handle = await client.resource("files.open", ["/data/report.csv"])
```

Use `resource` when payloads are too large to inline, you want to avoid copying data across function boundaries, or the data lives externally.

## Choosing the Right Primitive

| Situation                                        | Primitive  |
|--------------------------------------------------|------------|
| Simple function call, single return value        | `call`     |
| Notification or event with no response needed    | `cast`     |
| Server pushing a sequence of values              | `stream`   |
| Two-way interactive session                      | `channel`  |
| Multiple independent calls, minimize round trips | `batch`    |
| Large or external data                           | `resource` |

## Log

Every adapter includes a `log()` method that forwards structured log records to the runtime's log sink. Fire-and-forget, same as cast.

```typescript
await client.log("info", "my-app", "server started", { version: "1.0" });
```

```python
await client.log("info", "my-app", "server started", {"version": "1.0"})
```

## Next Steps

::: grids
::: grid
::: button "Schema" ./schema.md icon:file-text
:::
::: grid
::: button "Transports" ./transports.md icon:radio
:::
::: grid
::: button "Code Generation" ./codegen.md icon:cpu
:::
::: grid
::: button "Examples" ./examples.md icon:terminal
:::
:::
