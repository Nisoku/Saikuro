---
title: "Examples"
description: "Real patterns for multi-language systems with Saikuro"
---

Real patterns for building multi-language systems with Saikuro.

## Table of Contents

- [TypeScript Provider, Python Caller](#typescript-provider-python-caller)
- [Python Provider, TypeScript Caller](#python-provider-typescript-caller)
- [Event Streaming Across Languages](#event-streaming-across-languages)
- [Bidirectional Chat Channel](#bidirectional-chat-channel)
- [Capability-Gated Admin Functions](#capability-gated-admin-functions)
- [Batch Calls for Bulk Operations](#batch-calls-for-bulk-operations)
- [Testing with In-Memory Transport](#testing-with-in-memory-transport)
- [Rust Runtime Provider](#rust-runtime-provider)

---

## TypeScript Provider, Python Caller

A TypeScript service exposes user management. Python handles a background job that needs user data.

**TypeScript provider:**

```typescript
// services/users/provider.ts
import { Provider } from 'saikuro';
import { db } from './db';

const provider = new Provider({ namespace: 'users' });

provider.register('getById', async (id: string) => {
  const user = await db.users.findById(id);
  if (!user) throw new Error(`User ${id} not found`);
  return user;
});

provider.register('list', async (options: { page: number; limit: number }) => {
  const { items, total } = await db.users.paginate(options);
  return { items, total, page: options.page };
});

provider.register('deactivate', async (id: string) => {
  await db.users.update(id, { active: false });
  return { ok: true };
});

await provider.serve();
```

**Python caller:**

```python
# jobs/cleanup.py
import asyncio
from saikuro import Client

async def deactivate_inactive_users():
    client = Client()
    await client.connect()

    # Get all users (paginated)
    page = 0
    while True:
        result = await client.call('users.list', [{'page': page, 'limit': 100}])
        
        for user in result['items']:
            if not user['lastSeen'] or is_stale(user['lastSeen']):
                await client.call('users.deactivate', [user['id']])
                print(f"Deactivated {user['id']}")
        
        if len(result['items']) < 100:
            break
        page += 1

asyncio.run(deactivate_inactive_users())
```

---

## Python Provider, TypeScript Caller

A Python data science service exposes model inference. A TypeScript API calls it.

**Python provider:**

```python
# ml/inference/provider.py
import asyncio
from saikuro import Provider
import numpy as np

provider = Provider(namespace='model')
model = load_model('./weights.pt')

@provider.register('predict')
async def predict(features: list[float]) -> dict:
    arr = np.array(features)
    score = model.predict(arr)
    return {'score': float(score), 'label': classify(score)}

@provider.register('batch_predict')
async def batch_predict(items: list[list[float]]) -> list[dict]:
    arrs = np.array(items)
    scores = model.predict_batch(arrs)
    return [{'score': float(s), 'label': classify(s)} for s in scores]

asyncio.run(provider.serve())
```

**TypeScript caller:**

```typescript
// api/routes/classify.ts
import { Client } from 'saikuro';

const client = new Client();
await client.connect();

export async function classifyRequest(req, res) {
  const { features } = req.body;
  
  const result = await client.call('model.predict', [features]);
  
  res.json({
    score: result.score,
    label: result.label
  });
}
```

The TypeScript API doesn't care that the model is Python. It just calls `model.predict` and gets a result.

---

## Event Streaming Across Languages

A Rust service produces real-time events. TypeScript and Python consumers subscribe to them.

**Rust provider:**

```rust
use saikuro::{Provider, Result};
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<()> {
    let mut provider = Provider::new("events");

    provider.register_stream("subscribe", |filter: String| async move {
        let stream = create_event_stream(&filter);
        stream.map(|e| Ok(e))
    });

    provider.serve().await
}
```

**TypeScript consumer:**

```typescript
const client = new Client();
await client.connect();

console.log('Listening for errors...');

for await (const event of client.stream('events.subscribe', ['error'])) {
  console.log(`[${event.timestamp}] ${event.message}`);
  await alerting.notify(event);
}
```

**Python consumer (same stream, different process):**

```python
client = Client()
await client.connect()

async for event in client.stream('events.subscribe', ['error']):
    await metrics.increment('errors', tags={'source': event['source']})
    await pagerduty.trigger(event)
```

Both consumers get the same stream. They're independent, so one slow consumer doesn't affect the other.

---

## Bidirectional Chat Channel

A Python backend manages chat rooms. TypeScript browser clients connect via WebSocket.

**Python provider:**

```python
from saikuro import Provider
from collections import defaultdict
import asyncio

provider = Provider(namespace='chat')
rooms: dict[str, list] = defaultdict(list)

@provider.register_channel('join')
async def join_room(args, chan):
    room_id = args[0]['roomId']
    username = args[0]['username']
    
    rooms[room_id].append(chan)
    
    # Broadcast join notification
    await broadcast(room_id, {'type': 'join', 'user': username}, exclude=chan)
    
    try:
        async for msg in chan.incoming():
            if msg['type'] == 'message':
                await broadcast(room_id, {
                    'type': 'message',
                    'user': username,
                    'text': msg['text'],
                })
    finally:
        rooms[room_id].remove(chan)
        await broadcast(room_id, {'type': 'leave', 'user': username})

async def broadcast(room_id, msg, exclude=None):
    for chan in list(rooms[room_id]):
        if chan is not exclude:
            try:
                await chan.send(msg)
            except Exception:
                rooms[room_id].remove(chan)

await provider.serve()
```

**TypeScript browser client:**

```typescript
const client = new Client({
  transport: 'websocket',
  url: 'ws://localhost:7700'
});

await client.connect();

const chan = await client.channel('chat.join', [{
  roomId: 'general',
  username: 'alice'
}]);

// Send messages
sendButton.onclick = () => {
  chan.send({ type: 'message', text: input.value });
};

// Receive messages
for await (const msg of chan) {
  if (msg.type === 'message') {
    appendMessage(msg.user, msg.text);
  } else if (msg.type === 'join') {
    appendNotice(`${msg.user} joined`);
  }
}
```

---

## Capability-Gated Admin Functions

Admin functions that require a capability token.

**Schema:**

```json
{
  "version": 1,
  "namespaces": {
    "admin": {
      "functions": {
        "purge_queue": {
          "args": ["string"],
          "returns": "i32",
          "visibility": "internal",
          "capabilities": ["admin.write"]
        },
        "get_stats": {
          "args": [],
          "returns": "Stats",
          "visibility": "internal",
          "capabilities": ["admin.read"]
        }
      }
    }
  }
}
```

**TypeScript provider:**

```typescript
const provider = new Provider({ namespace: 'admin' });

provider.register('purge_queue', async (queueName: string): Promise<number> => {
  const count = await queue.purge(queueName);
  return count;
});

provider.register('get_stats', async () => {
  return await metrics.snapshot();
});

await provider.serve();
```

**Authorized caller:**

```typescript
const client = new Client({
  capabilities: { token: process.env.ADMIN_TOKEN }
});

await client.connect();

// Works: token grants admin.write
const purged = await client.call('admin.purge_queue', ['dead-letter']);
console.log(`Purged ${purged} messages`);
```

**Unauthorized caller:**

```typescript
const client = new Client();  // No token
await client.connect();

// Throws: CapabilityDenied
await client.call('admin.purge_queue', ['dead-letter']);
```

---

## Batch Calls for Bulk Operations

Load several resources in one round trip.

```typescript
const client = new Client();
await client.connect();

// Instead of three sequential calls...
// const user = await client.call('users.getById', ['u1']);
// const prefs = await client.call('prefs.getForUser', ['u1']);
// const notifications = await client.call('notifications.getUnread', ['u1']);

// ...one batch:
const [user, prefs, notifications] = await client.batch([
  { target: 'users.getById', args: ['u1'] },
  { target: 'prefs.getForUser', args: ['u1'] },
  { target: 'notifications.getUnread', args: ['u1'] },
]);

// All three results available immediately
renderProfile({ user, prefs, notifications });
```

---

## Testing with In-Memory Transport

Use `InMemoryTransport.pair()` for fast, isolated tests with no runtime process.

```typescript
// math.test.ts
import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { Provider, Client, InMemoryTransport } from 'saikuro';

describe('math provider', () => {
  let provider: Provider;
  let client: Client;

  beforeEach(async () => {
    const [pt, ct] = InMemoryTransport.pair();

    provider = new Provider({ namespace: 'math', transport: pt });
    provider.register('add', (a: number, b: number) => a + b);
    provider.register('multiply', (a: number, b: number) => a * b);
    await provider.serve();

    client = new Client({ transport: ct });
    await client.connect();
  });

  afterEach(async () => {
    await client.disconnect();
    await provider.stop();
  });

  it('adds two numbers', async () => {
    expect(await client.call('math.add', [1, 2])).toBe(3);
  });

  it('multiplies two numbers', async () => {
    expect(await client.call('math.multiply', [6, 7])).toBe(42);
  });

  it('handles errors from the provider', async () => {
    provider.register('divide', (a: number, b: number) => {
      if (b === 0) throw new Error('division by zero');
      return a / b;
    });

    await expect(client.call('math.divide', [1, 0]))
      .rejects.toThrow('division by zero');
  });
});
```

```python
# test_math.py
import pytest
from saikuro import Provider, Client, InMemoryTransport

@pytest.fixture
async def math_client():
    provider_t, client_t = InMemoryTransport.pair()

    provider = Provider(namespace='math', transport=provider_t)

    @provider.register('add')
    def add(a: int, b: int) -> int:
        return a + b

    await provider.serve()

    client = Client(transport=client_t)
    await client.connect()

    yield client

    await client.disconnect()
    await provider.stop()

@pytest.mark.asyncio
async def test_add(math_client):
    result = await math_client.call('math.add', [1, 2])
    assert result == 3
```

---

## Rust Runtime Provider

High-throughput or latency-sensitive work in Rust, called from other languages.

```rust
// src/main.rs
use saikuro::{Provider, Result};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<()> {
    let mut provider = Provider::new("index");

    // Build an in-memory search index
    let index = build_index();

    provider.register("search", move |query: String| {
        let index = index.clone();
        async move {
            let results = index.search(&query);
            Ok(results)
        }
    });

    provider.register("bulk_insert", move |items: Vec<HashMap<String, String>>| {
        let index = index.clone();
        async move {
            let count = index.bulk_insert(items).await?;
            Ok(count)
        }
    });

    println!("index provider ready");
    provider.serve().await
}
```

**Python caller (doing the heavy lifting in Rust):**

```python
client = Client()
await client.connect()

# Insert from Python, search from Python, but the index lives in Rust
await client.cast('index.bulk_insert', [documents])

results = await client.call('index.search', ['saikuro cross-language'])
for r in results:
    print(r['title'])
```

The Python code is clean. The performance-critical indexing and search is Rust. Saikuro handles the boundary.
