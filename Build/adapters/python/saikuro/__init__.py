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
from .provider import SaikuroProvider, register_function
from .envelope import Envelope, ResponseEnvelope, InvocationType
from .error import (
    SaikuroError,
    CapabilityDeniedError,
    FunctionNotFoundError,
    InvalidArgumentsError,
    TransportError,
    TimeoutError as SaikuroTimeoutError,
    ProviderError,
)
from .stream import SaikuroStream, SaikuroChannel
from .schema import SchemaBuilder, FunctionDef, ArgDef
from .transport import InMemoryTransport

__version__ = "0.1.0"
__all__ = [
    "SaikuroClient",
    "SaikuroProvider",
    "register_function",
    "Envelope",
    "ResponseEnvelope",
    "InvocationType",
    "SaikuroError",
    "CapabilityDeniedError",
    "FunctionNotFoundError",
    "InvalidArgumentsError",
    "TransportError",
    "SaikuroTimeoutError",
    "ProviderError",
    "SaikuroStream",
    "SaikuroChannel",
    "SchemaBuilder",
    "FunctionDef",
    "ArgDef",
    "InMemoryTransport",
]
