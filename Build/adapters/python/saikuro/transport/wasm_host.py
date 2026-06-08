"""WASM host transport using BroadcastChannel via Pyodide's js module."""

import asyncio
import secrets
import time
from typing import Optional

import msgpack

from saikuro.transport.base import BaseTransport


def _generate_id() -> str:
    now = int(time.time() * 1000) & 0xFFFFFFFF
    rand = secrets.randbits(32)
    return f"{now:08x}{rand:08x}"


class WasmHostTransport(BaseTransport):
    """Transport over BroadcastChannel API in Pyodide WASM environment.

    Performs the connect/accept handshake with the runtime:
      1. creates a private BroadcastChannel ``<channel>:<id>``
      2. posts ``{type: "connect", id: <id>}`` on the base channel
      3. waits for ``{type: "accept", id: <id>}`` on the private channel
      4. all subsequent traffic uses the private channel (MessagePack)
    """

    def __init__(self, channel_name: str = "saikuro"):
        self._channel_name = channel_name
        self._bc = None
        self._queue: asyncio.Queue = asyncio.Queue()
        self._closed = False
        self._handler = None

    async def connect(self):
        try:
            from js import BroadcastChannel, Object
        except ImportError:
            raise RuntimeError("WasmHostTransport requires Pyodide js module")

        conn_id = _generate_id()
        private_name = f"{self._channel_name}:{conn_id}"
        private_channel = BroadcastChannel.new(private_name)

        accept_event = asyncio.Event()

        def _on_accept(event):
            try:
                d = event.data
                if d.type == "accept" and d.id == conn_id:
                    accept_event.set()
            except Exception:
                pass

        from pyodide.ffi import create_proxy

        accept_proxy = create_proxy(_on_accept)
        private_channel.addEventListener("message", accept_proxy)

        # Send connect as a plain JS object, the Rust listener uses
        # js_sys::Reflect::get to extract "type" and "id" fields.
        base = BroadcastChannel.new(self._channel_name)
        connect_msg = Object.new()
        connect_msg.type = "connect"
        connect_msg.id = conn_id
        base.postMessage(connect_msg)
        base.close()

        try:
            await asyncio.wait_for(accept_event.wait(), timeout=10.0)
        except asyncio.TimeoutError:
            private_channel.close()
            accept_proxy.destroy()
            raise RuntimeError("WasmHostTransport: connect timeout")

        private_channel.removeEventListener("message", accept_proxy)
        accept_proxy.destroy()
        self._bc = private_channel
        self._handler = create_proxy(self._on_message)
        private_channel.addEventListener("message", self._handler)

    def _on_message(self, event):
        data = _get_data(event)
        if data is None:
            return
        try:
            decoded = msgpack.unpackb(data)
            self._queue.put_nowait(decoded)
        except Exception:
            pass

    async def send(self, obj: dict) -> None:
        if self._bc is None:
            raise RuntimeError("WasmHostTransport: not connected")
        from js import Uint8Array

        data = Uint8Array.new(msgpack.packb(obj))
        self._bc.postMessage(data)

    async def recv(self) -> Optional[dict]:
        if self._closed:
            return None
        item = await self._queue.get()
        if item is None:
            self._closed = True
            return None
        return item

    async def close(self) -> None:
        if not self._closed:
            self._closed = True
            await self._queue.put(None)
        if self._bc is not None:
            if self._handler is not None:
                try:
                    self._bc.removeEventListener("message", self._handler)
                    self._handler.destroy()
                except Exception:
                    pass
                self._handler = None
            self._bc.close()
            self._bc = None


def _get_data(event):
    """Extract bytes from a Pyodide ``message`` event."""
    data = event.data
    if data is None:
        return None
    if isinstance(data, (bytes, bytearray)):
        return bytes(data)
    if hasattr(data, "to_bytes"):
        return data.to_bytes()
    # Handle JS ArrayBuffer (sent by Rust send_buffer)
    if hasattr(data, "byteLength"):
        from js import Uint8Array

        return Uint8Array.new(data).to_bytes()
    return None
