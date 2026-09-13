"""
Python adapter for the Saikuro cross-language invocation fabric.

Handles MessagePack serialisation, transport selection, schema announcement,
handler registration, error propagation, and stream/channel iteration.

Usage (client)::

    async with SaikuroClient.connect("unix:///tmp/saikuro.sock") as client:
        result = await client.call("math.add", [1, 2])

Usage (provider)::

    provider = SaikuroProvider("math")

    @provider.register("add")
    async def add(a: int, b: int) -> int:
        return a + b

    await provider.serve("unix:///tmp/saikuro.sock")
"""

from .client import SaikuroClient
from .envelope import Envelope, InvocationType, ResponseEnvelope
from .error import (
    CapabilityDeniedError,
    FunctionNotFoundError,
    InvalidArgumentsError,
    ProviderError,
    SaikuroError,
    TransportError,
)
from .error import (
    TimeoutError as SaikuroTimeoutError,
)
from .provider import SaikuroProvider, register_function
from .schema import ArgDef, FunctionDef, SchemaBuilder
from .stream import SaikuroChannel, SaikuroStream
from .transport import InMemoryTransport

__version__ = "0.1.0"
__all__ = [
    "ArgDef",
    "CapabilityDeniedError",
    "Envelope",
    "FunctionDef",
    "FunctionNotFoundError",
    "InMemoryTransport",
    "InvalidArgumentsError",
    "InvocationType",
    "ProviderError",
    "ResponseEnvelope",
    "SaikuroChannel",
    "SaikuroClient",
    "SaikuroError",
    "SaikuroProvider",
    "SaikuroStream",
    "SaikuroTimeoutError",
    "SchemaBuilder",
    "TransportError",
    "register_function",
]
