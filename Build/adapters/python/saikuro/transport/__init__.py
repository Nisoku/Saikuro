"""
Transport abstraction for the Python Saikuro adapter.

Implementations:
  - UnixSocketTransport  (same-machine, Unix only)
  - TcpTransport         (cross-machine)
  - WebSocketTransport   (ws:// or wss://)
  - InMemoryTransport    (in-process testing)
  - WasmHostTransport    (Pyodide WASM environment)
"""

from __future__ import annotations

import threading

from saikuro.transport.base import BaseTransport
from saikuro.transport.memory import InMemoryTransport
from saikuro.transport.unix import UnixSocketTransport
from saikuro.transport.tcp import TcpTransport
from saikuro.transport.websocket import WebSocketTransport
from saikuro.transport.wasm_host import WasmHostTransport


__all__ = [
    "BaseTransport",
    "InMemoryTransport",
    "UnixSocketTransport",
    "TcpTransport",
    "WebSocketTransport",
    "WasmHostTransport",
    "make_transport",
    "reset_transport_factory",
]


class _TransportFactory:
    """Type-safe factory for constructing transports from address strings."""

    def __init__(self) -> None:
        self._channels: dict[str, InMemoryTransport] = {}
        self._lock = threading.Lock()

    def __call__(self, address: str) -> BaseTransport:
        """
        Construct the best transport for `address`.

        Address format:
          - ``unix:///path/to/socket``  - Unix domain socket
          - ``tcp://host:port``         - TCP
          - ``ws://host:port/path``     - WebSocket (plain)
          - ``wss://host:port/path``    - WebSocket (TLS)
          - ``memory://``               - In-memory (testing only; creates default channel)
          - ``memory://channel-name``   - In-memory with named channel

        In-memory channels work as pairs:
          - First call to ``memory://foo`` creates a pair and returns side A
          - Second call to ``memory://foo`` returns side B, connected to A
          - For tests, prefer ``InMemoryTransport.pair()`` for direct control
        """
        if address.startswith("memory://"):
            name = (
                address[len("memory://") :]
                if len(address) > len("memory://")
                else "default"
            )
            with self._lock:
                if name in self._channels:
                    return self._channels.pop(name)
                else:
                    a, b = InMemoryTransport.pair()
                    self._channels[name] = b
                    return a

        if address.startswith("unix://"):
            path = address[len("unix://") :]
            return UnixSocketTransport(path)
        if address.startswith("tcp://"):
            rest = address[len("tcp://") :]
            if rest.startswith("["):
                bracket_end = rest.find("]")
                if (
                    bracket_end == -1
                    or bracket_end + 1 >= len(rest)
                    or rest[bracket_end + 1] != ":"
                ):
                    raise ValueError(f"invalid TCP address format: {address!r}")
                host = rest[1:bracket_end]
                port_str = rest[bracket_end + 2 :]
            else:
                host, _, port_str = rest.rpartition(":")
            return TcpTransport(host, int(port_str))
        if address.startswith("ws://") or address.startswith("wss://"):
            return WebSocketTransport(address)
        if address.startswith("wasm-host://"):
            channel = address[len("wasm-host://") :]
            return WasmHostTransport(channel)
        raise ValueError(
            f"unsupported transport address: {address!r}\n"
            "Supported schemes: unix://, tcp://, ws://, wss://, memory://, wasm-host://"
        )

    def reset(self) -> None:
        """Clear all cached in-memory channels.  Call between tests for isolation."""
        with self._lock:
            self._channels.clear()

    def _remove_channel(self, transport: InMemoryTransport) -> None:
        """Remove *transport* from the registry if present."""
        with self._lock:
            for name, t in list(self._channels.items()):
                if t is transport:
                    self._channels.pop(name, None)
                    break


make_transport = _TransportFactory()
reset_transport_factory = make_transport.reset
