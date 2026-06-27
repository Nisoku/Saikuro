---
type: concept
title: "Python Examples"
description: "Python adapter usage patterns"
source: "https://nisoku.org/Saikuro/adapters/python/examples/"
path: /adapters/python/examples/
updated: 2026-06-27
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-06-27T14:04:05.290Z"
---
---
title: "Python Examples"
description: "Python adapter usage patterns"
---

## Full Provider

```python
import asyncio
from saikuro import SaikuroProvider

provider = SaikuroProvider("math")

@provider.register("add")
def add(a: int, b: int) -> int:
    return a + b

@provider.register("multiply")
def multiply(a: int, b: int) -> int:
    return a * b

@provider.register("fibonacci")
async def fibonacci(n: int):
    a, b = 0, 1
    for _ in range(n):
        yield a
        a, b = b, a + b

asyncio.run(provider.serve("unix:///tmp/saikuro.sock"))
```

## Full Client

```python
import asyncio
from saikuro import SaikuroClient

async def main():
    async with SaikuroClient.connect("unix:///tmp/saikuro.sock") as client:
        # Call
        result = await client.call("math.add", [1, 2])
        print(f"add: {result}")

        # Stream
        async for n in await client.stream("math.fibonacci", [10]):
            print(f"fib: {n}")

        # Batch
        results = await client.batch([
            ("math.add", [3, 4]),
            ("math.multiply", [5, 6]),
        ])
        print(f"batch: {results}")

asyncio.run(main())
```

## Testing with InMemory

```python
import pytest
from saikuro import SaikuroProvider, SaikuroClient, InMemoryTransport

@pytest.mark.asyncio
async def test_math_provider():
    pt, ct = InMemoryTransport.pair()

    provider = SaikuroProvider("math")

    @provider.register("add")
    def add(a: int, b: int) -> int:
        return a + b

    await provider.serve_on(pt)

    async with SaikuroClient.open_on(ct) as client:
        assert await client.call("math.add", [1, 2]) == 3
```
