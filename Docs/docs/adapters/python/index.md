---
title: "Python Adapter"
description: "Async Python adapter for Saikuro"
---

The Python adapter provides async provider and client APIs for Saikuro. Python 3.11+ is required.

## Install

```bash
pip install @nisoku/saikuro
```

For WebSocket support:

```bash
pip install "saikuro[websocket]"
```

## Client

```python
import asyncio
from saikuro import Client

async def main():
    client = await Client.connect("tcp://127.0.0.1:7700")
    result = await client.call("math.add", [1, 2])
    print(result)

asyncio.run(main())
```

## Provider

```python
import asyncio
from saikuro import Provider

async def main():
    provider = Provider("math")

    @provider.register("add")
    async def add(args):
        a, b = args
        return a + b

    await provider.serve("tcp://127.0.0.1:7700")

asyncio.run(main())
```

## Next Steps

- [Code Generation](../../guide/codegen): Generate Python stubs from schema
- [Python API Reference](./api-reference): Client and provider method reference
- [Python examples](./examples): Python with TypeScript and Rust patterns