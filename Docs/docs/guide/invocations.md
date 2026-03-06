---
title: "Invocation Primitives"
description: "The six ways to communicate across language boundaries with Saikuro"
---

Saikuro gives you six primitives for cross-language communication. They cover the full range of patterns you'll actually need, so don't force everything into request/response when a stream or channel is the right tool.

## Call

Request/response. The most common primitive. The caller sends arguments and waits for a single return value.

```typescript
// TypeScript caller
const result = await client.call('math.add', [1, 2]);
// result === 3
```

```python
# Python caller
result = await client.call('math.add', [1, 2])
# result == 3
```

```csharp
// C# caller
var result = await client.CallAsync<int>("math.add", new object[] { 1, 2 });
// result == 3
```

Use `call` when you need a response and the response is a single value.

## Cast

Fire and forget. The caller sends a message and does not wait for a response. The runtime delivers it best-effort.

```typescript
// TypeScript
await client.cast('log.write', [{ level: 'info', message: 'started' }]);
```

```python
# Python
await client.cast('log.write', [{'level': 'info', 'message': 'started'}])
```

Use `cast` for notifications, audit events, log writes, and anything else where you don't need to know if it succeeded. It's faster than `call` because there's no response to wait for.

## Stream

One-way sequence of messages. The provider sends a series of values and the caller iterates over them.

```typescript
// TypeScript: consume a stream
for await (const event of client.stream('events.subscribe', [])) {
  console.log(event);
}
```

```python
# Python: consume a stream
async for event in client.stream('events.subscribe', []):
    print(event)
```

```csharp
// C#: consume a stream
await foreach (var item in client.StreamAsync<Event>("events.subscribe", Array.Empty<object>()))
{
    Console.WriteLine(item);
}
```

**Provider side (TypeScript):**

```typescript
provider.registerStream('events.subscribe', async function* (filter) {
  while (true) {
    const event = await waitForNextEvent(filter);
    yield event;
  }
});
```

Use `stream` for subscriptions, paginated results, log tailing, progress reporting, and anything where the response is multiple values over time.

::: callout info
Streams are ordered. Messages arrive at the caller in the order the provider sent them.
:::

## Channel

Bidirectional stream with backpressure. Both sides can send and receive. The runtime applies flow control so neither side can overwhelm the other.

```typescript
// TypeScript
const chan = await client.channel('chat.session', [{ room: 'general' }]);

// Send messages
await chan.send({ type: 'message', text: 'hello' });

// Receive messages
for await (const msg of chan) {
  console.log(msg.text);
}

// Close when done
await chan.close();
```

```python
# Python
async with client.channel('chat.session', [{'room': 'general'}]) as chan:
    await chan.send({'type': 'message', 'text': 'hello'})
    async for msg in chan:
        print(msg['text'])
```

**Provider side:**

```typescript
provider.registerChannel('chat.session', async (args, chan) => {
  const { room } = args[0];
  
  // Read from caller
  for await (const msg of chan.incoming()) {
    // Broadcast to room, then echo back
    await broadcastToRoom(room, msg);
    await chan.send({ type: 'ack', id: msg.id });
  }
});
```

Use `channel` for interactive sessions, bidirectional data transfer, or any pattern where both sides need to talk simultaneously.

::: callout warning
Channels implement backpressure. If the consumer falls behind, sends will block until there's capacity. Design your provider to handle slow consumers gracefully.
:::

## Batch

Multiple calls in one envelope. Useful when you have several independent calls and want to minimize round trips.

```typescript
// TypeScript
const results = await client.batch([
  { target: 'math.add', args: [1, 2] },
  { target: 'math.multiply', args: [3, 4] },
  { target: 'text.upper', args: ['hello'] },
]);

// results[0] === 3
// results[1] === 12
// results[2] === 'HELLO'
```

```python
# Python
results = await client.batch([
    {'target': 'math.add', 'args': [1, 2]},
    {'target': 'math.multiply', 'args': [3, 4]},
    {'target': 'text.upper', 'args': ['hello']},
])
```

Batch calls can span namespaces. Each call in the batch is routed independently. Results come back in the same order as the requests.

Use `batch` when you have several independent calls you'd otherwise make sequentially.

## Resource

An opaque handle to large or external data. Instead of sending a 50MB file inline in an envelope, you send a resource reference and the receiver fetches it through the runtime.

```typescript
// TypeScript: upload and get a handle
const handle = await client.createResource(largeBuffer, { contentType: 'image/png' });

// Pass the handle to a function that needs the data
const result = await client.call('images.resize', [handle, { width: 800 }]);

// The provider accesses the data through the handle, not inline
```

**Provider side:**

```typescript
provider.register('images.resize', async (handle, options) => {
  const data = await handle.read();  // Fetches from the runtime
  return resize(data, options.width);
});
```

Use `resource` when:
- Payloads are too large to include inline
- You want to avoid copying data across multiple function boundaries
- The data lives externally and the provider should access it on demand

## Choosing the Right Primitive

| Situation | Primitive |
| --------- | --------- |
| Simple function call, single return value | `call` |
| Notification or event with no response needed | `cast` |
| Server pushing a sequence of values | `stream` |
| Two-way interactive session | `channel` |
| Multiple independent calls, minimize round trips | `batch` |
| Large or external data | `resource` |

## Next Steps

- [Schema](./schema): Declare your functions, types, and capabilities
- [Transports](./transports): How the protocol moves between processes
- [Examples](./examples): Real patterns using all six primitives
