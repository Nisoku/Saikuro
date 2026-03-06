"""
Math provider example (Python).

Exposes four arithmetic functions under the "math" namespace:
  math.add, math.subtract, math.multiply, math.divide

Run with:
  pip install -e ../../../Build/adapters/python
  python main.py [address]

where `address` defaults to `tcp://127.0.0.1:7700`.
"""

import asyncio
import sys

from saikuro import SaikuroProvider, ProviderError


def main() -> None:
    address = sys.argv[1] if len(sys.argv) > 1 else "tcp://127.0.0.1:7700"
    asyncio.run(_serve(address))


async def _serve(address: str) -> None:
    provider = SaikuroProvider("math")

    @provider.register("add")
    def add(a: float, b: float) -> float:
        return a + b

    @provider.register("subtract")
    def subtract(a: float, b: float) -> float:
        return a - b

    @provider.register("multiply")
    def multiply(a: float, b: float) -> float:
        return a * b

    @provider.register("divide")
    def divide(a: float, b: float) -> float:
        if b == 0:
            raise ProviderError("ProviderError", "division by zero")
        return a / b

    print(f"math provider listening on {address}")
    await provider.serve(address)


if __name__ == "__main__":
    main()
