"""
Math example: one provider and one client over a selectable transport.

Run with:
  pip install -e ../../../Build/adapters/python
  python main.py                              # in-memory (default)
  python main.py --transport tcp              # loopback TCP
  python main.py --transport tcp --addr 127.0.0.1:7700
"""

from __future__ import annotations

import argparse
import asyncio
import contextlib
import sys

from saikuro import (
    InMemoryTransport,
    ProviderError,
    SaikuroClient,
    SaikuroError,
    SaikuroProvider,
)
from saikuro.transport import BaseTransport, TcpTransport

LOOPBACK = "127.0.0.1"
"""Default listen host."""

DEFAULT_PORT = 0
"""Default listen port. 0 asks the OS for a free port."""


def math_provider() -> SaikuroProvider:
    """Build the shared math schema, used for every transport."""
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

    return provider


async def run_demo(client: SaikuroClient) -> None:
    """Drive the shared client demo. Identical for every transport."""
    # call

    total = await client.call("math.add", [10, 32])
    print(f"math.add(10, 32) = {total}")
    assert total == 42, f"expected 42, got {total}"

    difference = await client.call("math.subtract", [100, 58])
    print(f"math.subtract(100, 58) = {difference}")
    assert difference == 42, f"expected 42, got {difference}"

    product = await client.call("math.multiply", [6, 7])
    print(f"math.multiply(6, 7) = {product}")
    assert product == 42, f"expected 42, got {product}"

    quotient = await client.call("math.divide", [84, 2])
    print(f"math.divide(84, 2) = {quotient}")
    assert quotient == 42, f"expected 42, got {quotient}"

    # cast (fire-and-forget)

    await client.cast("math.add", [1, 1])
    print("cast sent (no response expected)")

    # batch

    batch = await client.batch(
        [
            ("math.add", [1, 2]),
            ("math.multiply", [3, 4]),
        ]
    )
    print(f"batch [add(1,2), multiply(3,4)] = {batch}")

    # error handling

    try:
        await client.call("math.divide", [1, 0])
    except SaikuroError as err:
        print(f"divide by zero caught: [{err.code}] {err.message}")


async def serve_over_pair(
    provider_transport: BaseTransport,
    client_transport: BaseTransport,
) -> None:
    """Serve the shared provider and drive the shared demo over a transport pair."""
    serve_task = asyncio.ensure_future(math_provider().serve_on(provider_transport))
    client = await SaikuroClient.open_on(client_transport)

    try:
        await run_demo(client)
    finally:
        await client.close()
        serve_task.cancel()
        with contextlib.suppress(asyncio.CancelledError):
            await serve_task


async def run_in_memory() -> None:
    """Wire provider and client over a paired in-memory transport."""
    print("transport: in-memory")
    provider_transport, client_transport = InMemoryTransport.pair()
    await serve_over_pair(provider_transport, client_transport)


async def run_tcp(host: str, port: int) -> None:
    """Bind a loopback TCP listener, serve the accepted connection, and dial it."""
    accepted: asyncio.Future[tuple[asyncio.StreamReader, asyncio.StreamWriter]] = (
        asyncio.get_running_loop().create_future()
    )

    async def on_connection(
        reader: asyncio.StreamReader,
        writer: asyncio.StreamWriter,
    ) -> None:
        if not accepted.done():
            accepted.set_result((reader, writer))

    server = await asyncio.start_server(on_connection, host, port)
    # The listener must be closed even if the dial or the accept fails
    client_transport: TcpTransport | None = None
    try:
        sockets = server.sockets or ()
        bound_port = sockets[0].getsockname()[1] if sockets else port
        print(f"transport: tcp (provider listening on {host}:{bound_port})")

        client_transport = TcpTransport(host, bound_port)
        # Dial before awaiting the accept: the server cannot see a connection
        # until the client reaches out, so the reverse order deadlocks. `open_on`
        # would dial anyway, but the connect is explicit to keep the ordering
        # obvious.
        await client_transport.connect()

        reader, writer = await accepted
        provider_transport = TcpTransport.from_stream(reader, writer)

        await serve_over_pair(provider_transport, client_transport)
    finally:
        if client_transport is not None:
            await client_transport.close()
        server.close()
        await server.wait_closed()


def parse_args(argv: list[str]) -> argparse.Namespace:
    """Parse the command line, defaulting to in-memory."""
    parser = argparse.ArgumentParser(
        prog="math",
        description="Saikuro math example (Python).",
    )
    parser.add_argument(
        "--transport",
        choices=("memory", "tcp"),
        default="memory",
        help="transport to wire the provider and client over (default: memory)",
    )
    parser.add_argument(
        "--addr",
        default=f"{LOOPBACK}:{DEFAULT_PORT}",
        metavar="HOST:PORT",
        help=f"address for the TCP provider listener (default: {LOOPBACK}:{DEFAULT_PORT})",
    )
    args = parser.parse_args(argv)

    if args.transport == "tcp":
        host, _, port_text = args.addr.rpartition(":")
        if not host or not port_text.isdigit():
            parser.error(f"invalid --addr '{args.addr}': expected HOST:PORT")
        args.addr = (host, int(port_text))

    return args


async def main(argv: list[str]) -> None:
    args = parse_args(argv)
    if args.transport == "memory":
        await run_in_memory()
    else:
        host, port = args.addr
        await run_tcp(host, port)
    print("all examples passed")


if __name__ == "__main__":
    # Ctrl-C is the documented way to stop this example.
    with contextlib.suppress(KeyboardInterrupt):
        asyncio.run(main(sys.argv[1:]))
