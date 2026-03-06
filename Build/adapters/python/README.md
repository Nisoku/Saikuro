# saikuro Python adapter

Python adapter for the [Saikuro](https://github.com/Nisoku/Saikuro) cross-language
IPC fabric. Requires Python 3.11+.

## Installation

```bash
pip install @nisoku/saikuro

# With WebSocket transport support:
pip install "saikuro[websocket]"
```

## Usage

### Client

```python
import asyncio
from saikuro import Client

async def main():
    client = await Client.connect("tcp://127.0.0.1:7700")
    result = await client.call("math.add", [1, 2])
    print(result)  # 3

asyncio.run(main())
```

### Provider

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

## Development

```bash
uv pip install -e ".[dev]"
uv pytest
```

## License

Apache-2.0
