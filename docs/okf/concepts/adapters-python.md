---
type: concept
title: "Python Adapter"
description: "Saikuro adapter for Python 3.11+"
source: "https://nisoku.org/Saikuro/adapters/python/"
path: /adapters/python/
updated: 2026-06-27
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-06-27T13:25:19.827Z"
---
---
title: "Python Adapter"
description: "Saikuro adapter for Python 3.11+"
---

The Python adapter provides async client and provider APIs for Python 3.11+.

## Installation

```bash
pip install saikuro
```

## Client API

```python
from saikuro import SaikuroClient, SaikuroChannel, SaikuroStream

# Connect via address string
async with SaikuroClient.connect("unix:///tmp/saikuro.sock") as client:
    # Call a function
    result = await client.call("math.add", [1, 2])

    # Fire-and-forget
    await client.cast("log.write", [{"level": "info", "message": "started"}])

    # Stream
    stream = await client.stream("events.subscribe", [])
    async for event in stream:
        print(event)

    # Channel
    async with client.channel("chat.session", [{"room": "general"}]) as chan:
        await chan.send({"text": "hello"})
        async for msg in chan:
            print(msg)

    # Batch
    results = await client.batch([
        ("math.add", [1, 2]),
        ("math.multiply", [3, 4]),
    ])

    # Resource
    handle = await client.resource("files.open", ["/data.csv"])

    # Log
    await client.log("info", "myapp", "started", {"version": "1.0"})

# Or manage lifecycle manually
client = SaikuroClient.from_transport(transport)
await client.connect()
# ...
await client.close()
```

## Provider API

```python
from saikuro import SaikuroProvider

provider = SaikuroProvider("math")

# Decorator registration
@provider.register("add")
def add(a: int, b: int) -> int:
    return a + b

@provider.register("divide", capabilities=["math.divide"])
def divide(a: float, b: float) -> float:
    if b == 0:
        raise ValueError("division by zero")
    return a / b

# Stream handler (async generator)
@provider.register("events.subscribe")
async def subscribe(topic: str):
    async for event in poll_events(topic):
        yield event

# Imperative registration
provider.register_function("multiply", lambda a, b: a * b)

# Serve
await provider.serve("unix:///tmp/saikuro.sock")
# Or serve on existing transport
await provider.serve_on(transport)
```

## Transport

```python
from saikuro import make_transport, InMemoryTransport

# Address-based
transport = make_transport("unix:///tmp/saikuro.sock")

# InMemory pair for testing
pt, ct = InMemoryTransport.pair()
```

See [Transports](../../guide/transports) for the full address format reference.

## Export Surface

```python
# Core
SaikuroClient, SaikuroProvider, SaikuroStream, SaikuroChannel

# Transports
BaseTransport, InMemoryTransport, UnixSocketTransport,
TcpTransport, WebSocketTransport, WasmHostTransport,
make_transport, reset_transport_factory

# Errors
SaikuroError, TransportError

# Envelope types
Envelope, InvocationType, ResourceHandle, LogLevel, LogRecord
```
