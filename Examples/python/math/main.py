"""
Math example: one provider and one client over a selectable transport.

Run with:
  pip install -e ../../../Build/adapters/python
  python main.py                                 # in-memory (default)
  python main.py --transport tcp                 # loopback TCP
  python main.py --transport tcp --addr 127.0.0.1:7700
  python main.py --transport unix                # loopback Unix socket
  python main.py --transport unix --addr /tmp/math.sock
  python main.py --transport ws                  # loopback WebSocket
  python main.py --transport ws --addr 127.0.0.1:7700

The socket modes host a provider and dial it in the same process. Use
--serve-only to listen without a local client, or --client-only to dial an
existing provider without listening:
  python main.py --transport tcp --addr 127.0.0.1:7700 --serve-only
  python main.py --transport tcp --addr 127.0.0.1:7700 --client-only
"""

from __future__ import annotations

import argparse
import asyncio
import contextlib
import os
import sys
from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from typing import Literal

from saikuro import (
    InMemoryTransport,
    ProviderError,
    SaikuroClient,
    SaikuroError,
    SaikuroProvider,
)
from saikuro.transport import (
    BaseTransport,
    TcpTransport,
    UnixSocketTransport,
    WebSocketListener,
)

TransportChoice = Literal["memory", "tcp", "unix", "ws"]
"""A transport this example can wire the provider and client over."""

Mode = Literal["both", "serve-only", "client-only"]
"""How the provider and client are wired for this run."""

DEFAULT_UNIX_PATH = "/tmp/saikuro-math.sock"
"""Default Unix domain socket path."""

DEFAULT_ADDR: dict[str, str] = {
    "memory": "",
    "tcp": "127.0.0.1:0",
    "unix": DEFAULT_UNIX_PATH,
    "ws": "127.0.0.1:0",
}
"""Per-transport ``--addr`` default. Port 0 asks the OS for a free port."""


@dataclass
class Options:
    """Parsed command line."""

    transport: TransportChoice
    addr: str
    mode: Mode

    def host_port(self) -> tuple[str, int]:
        """Split :attr:`addr` into ``(host, port)`` for tcp/ws."""
        return parse_host_port(self.addr)


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


async def serve_connection(transport: BaseTransport) -> None:
    """Serve the shared provider on one accepted connection."""
    try:
        await math_provider().serve_on(transport)
    except Exception as exc:  # noqa: BLE001 - one bad connection must not stop the listener
        print(f"provider: connection failed: {exc}")


async def run_in_memory() -> None:
    """Wire provider and client over a paired in-memory transport."""
    print("transport: in-memory")
    provider_transport, client_transport = InMemoryTransport.pair()
    serve_task = asyncio.ensure_future(math_provider().serve_on(provider_transport))
    client = await SaikuroClient.open_on(client_transport)
    try:
        await run_demo(client)
    finally:
        await client.close()
        serve_task.cancel()
        with contextlib.suppress(asyncio.CancelledError):
            await serve_task


def _bound_port(server: asyncio.Server, fallback: int) -> int:
    """The actual port an ``asyncio.Server`` is bound to, or `fallback`."""
    sockets = server.sockets or ()
    return sockets[0].getsockname()[1] if sockets else fallback


def _suffix(mode: Mode) -> str:
    """Suffix the provider-listening banner when running serve-only."""
    return ", serve-only" if mode == "serve-only" else ""


async def _serve_streams(
    *,
    make_transport: Callable[
        [asyncio.StreamReader, asyncio.StreamWriter], BaseTransport
    ],
    start_server: Callable[
        [Callable[[asyncio.StreamReader, asyncio.StreamWriter], Awaitable[None]]],
        Awaitable[asyncio.Server],
    ],
    endpoint: Callable[[asyncio.Server], tuple[str, str]],
    mode: Mode,
) -> None:
    """Bind a stream listener, serve each connection, and drive the demo.

    `endpoint` maps the bound server to ``(banner, client_url)`` so a port of 0
    reports and dials the port the OS actually chose.
    """

    async def on_connection(
        reader: asyncio.StreamReader, writer: asyncio.StreamWriter
    ) -> None:
        await serve_connection(make_transport(reader, writer))

    server = await start_server(on_connection)
    try:
        banner, client_url = endpoint(server)
        print(banner, flush=True)
        if mode == "serve-only":
            async with server:
                await server.serve_forever()
            return
        client = await SaikuroClient.connect(client_url)
        try:
            await run_demo(client)
        finally:
            await client.close()
    finally:
        server.close()
        await server.wait_closed()


async def run_tcp(options: Options) -> None:
    """Loopback TCP: bind a listener and either serve or dial it."""
    host, port = options.host_port()

    def endpoint(server: asyncio.Server) -> tuple[str, str]:
        bound = _bound_port(server, port)
        return (
            f"transport: tcp (provider listening on {host}:{bound}{_suffix(options.mode)})",
            f"tcp://{host}:{bound}",
        )

    await _serve_streams(
        make_transport=TcpTransport.from_stream,
        start_server=lambda on_connection: asyncio.start_server(
            on_connection, host, port
        ),
        endpoint=endpoint,
        mode=options.mode,
    )


async def run_unix(options: Options) -> None:
    """Loopback Unix socket: bind a listener and either serve or dial it."""
    path = options.addr
    # A socket file left by a previous run would make bind fail.
    with contextlib.suppress(FileNotFoundError):
        os.unlink(path)

    def endpoint(_server: asyncio.Server) -> tuple[str, str]:
        return (
            f"transport: unix (provider listening on {path}{_suffix(options.mode)})",
            f"unix://{path}",
        )

    try:
        await _serve_streams(
            make_transport=UnixSocketTransport.from_stream,
            start_server=lambda on_connection: asyncio.start_unix_server(
                on_connection, path
            ),
            endpoint=endpoint,
            mode=options.mode,
        )
    finally:
        with contextlib.suppress(FileNotFoundError):
            os.unlink(path)


async def _accept_one(listener: WebSocketListener) -> None:
    """Accept a single connection and serve the provider on it."""
    await serve_connection(await listener.accept())


async def _accept_forever(listener: WebSocketListener) -> None:
    """Accept connections until cancelled, serving each in the background."""
    while True:
        transport = await listener.accept()
        asyncio.ensure_future(serve_connection(transport))


async def run_ws(options: Options) -> None:
    """Loopback WebSocket: bind a listener and either serve or dial it."""
    host, port = options.host_port()
    listener = await WebSocketListener.bind(host=host, port=port)
    try:
        print(
            f"transport: ws (provider listening on {host}:{listener.port}"
            f"{_suffix(options.mode)})",
            flush=True,
        )
        if options.mode == "serve-only":
            await _accept_forever(listener)
            return
        serve_task = asyncio.ensure_future(_accept_one(listener))
        client = await SaikuroClient.connect(f"ws://{host}:{listener.port}")
        try:
            await run_demo(client)
        finally:
            await client.close()
            with contextlib.suppress(asyncio.TimeoutError, asyncio.CancelledError):
                await asyncio.wait_for(serve_task, timeout=1.0)
    finally:
        await listener.close()


def client_url(options: Options) -> str:
    """Build the client address for the selected transport."""
    if options.transport == "tcp":
        return f"tcp://{options.addr}"
    if options.transport == "unix":
        return f"unix://{options.addr}"
    if options.transport == "ws":
        if options.addr.startswith(("ws://", "wss://")):
            return options.addr
        return f"ws://{options.addr}"
    return "memory://"


async def run_client(options: Options) -> None:
    """Dial an existing provider and drive the shared demo."""
    url = client_url(options)
    print(f"transport: {options.transport} (client dialling {url})")
    client = await SaikuroClient.connect(url)
    try:
        await run_demo(client)
    finally:
        await client.close()


def parse_host_port(value: str) -> tuple[str, int]:
    """Split ``HOST:PORT`` into its parts, rejecting anything malformed."""
    host, separator, port_text = value.rpartition(":")
    if not separator or not host or not port_text.isdigit():
        raise ValueError("expected HOST:PORT")
    return host, int(port_text)


def parse_args(argv: list[str]) -> Options:
    """Parse the command line, defaulting to in-memory and per-transport addr."""
    parser = argparse.ArgumentParser(
        prog="math",
        description="Saikuro math example (Python).",
    )
    parser.add_argument(
        "--transport",
        choices=("memory", "tcp", "unix", "ws", "websocket"),
        default="memory",
        help="transport to wire the provider and client over (default: memory)",
    )
    parser.add_argument(
        "--addr",
        default=None,
        metavar="ADDR",
        help="HOST:PORT for tcp/ws, a filesystem path for unix",
    )
    mode_group = parser.add_mutually_exclusive_group()
    mode_group.add_argument(
        "--serve-only",
        action="store_true",
        help="listen without a local client",
    )
    mode_group.add_argument(
        "--client-only",
        action="store_true",
        help="dial an existing provider without listening",
    )

    args = parser.parse_args(argv)

    transport: TransportChoice = (
        "ws" if args.transport == "websocket" else args.transport
    )
    if args.serve_only:
        mode: Mode = "serve-only"
    elif args.client_only:
        mode = "client-only"
    else:
        mode = "both"

    if mode != "both" and transport == "memory":
        parser.error("--serve-only/--client-only need a socket transport")

    addr = args.addr if args.addr is not None else DEFAULT_ADDR[transport]
    if transport in ("tcp", "ws"):
        try:
            parse_host_port(addr)
        except ValueError as exc:
            parser.error(f"invalid --addr '{addr}': {exc}")

    return Options(transport=transport, addr=addr, mode=mode)


async def main(argv: list[str]) -> None:
    options = parse_args(argv)
    if options.transport == "memory":
        await run_in_memory()
    elif options.mode == "client-only":
        await run_client(options)
    elif options.transport == "tcp":
        await run_tcp(options)
    elif options.transport == "unix":
        await run_unix(options)
    else:
        await run_ws(options)
    print("all examples passed")


if __name__ == "__main__":
    # Ctrl-C is the documented way to stop this example.
    with contextlib.suppress(KeyboardInterrupt):
        asyncio.run(main(sys.argv[1:]))
