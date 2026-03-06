"""
Saikuro provider - register Python functions and serve them to the runtime.

A provider:
  1. Connects to (or embeds) a Saikuro runtime.
  2. Announces its schema.
  3. Receives invocation envelopes.
  4. Calls the matching Python function.
  5. Sends back response envelopes.

Example::

    provider = SaikuroProvider("math")

    @provider.register("add")
    def add(a: int, b: int) -> int:
        return a + b

    await provider.serve("unix:///tmp/saikuro.sock")
"""

from __future__ import annotations

import asyncio
import inspect
import logging
from typing import Any, Callable, Dict, List, Optional

from .envelope import Envelope, InvocationType, ResponseEnvelope, StreamControl
from .error import SaikuroError
from .schema import SchemaBuilder
from .transport import BaseTransport, make_transport

logger = logging.getLogger(__name__)

# A handler is any callable (sync or async) that accepts positional args and
# returns a value (or an async generator for streams).
Handler = Callable[..., Any]


class SaikuroProvider:
    """
    A Saikuro provider that exposes Python callables as invokable functions.

    One provider instance maps to one namespace.  Multiple providers for
    multiple namespaces can share the same transport connection by composing
    them into a single `SaikuroServer`.
    """

    def __init__(self, namespace: str) -> None:
        self._namespace = namespace
        self._handlers: Dict[str, Handler] = {}
        self._schema_builder = SchemaBuilder(namespace)

    @property
    def namespace(self) -> str:
        return self._namespace

    #  Registration

    def register(
        self,
        name: str,
        *,
        capabilities: Optional[List[str]] = None,
        doc: Optional[str] = None,
    ) -> Callable[[Handler], Handler]:
        """Decorator that registers a function under `name`.

        Works with both regular functions and async functions.
        Async generators are accepted for stream-returning functions.
        """

        def decorator(fn: Handler) -> Handler:
            full_name = f"{self._namespace}.{name}"
            self._handlers[name] = fn
            self._schema_builder.add_function(
                name=name,
                fn=fn,
                capabilities=capabilities or [],
                doc=doc or (fn.__doc__ or "").strip(),
            )
            logger.debug("registered handler %s", full_name)
            return fn

        return decorator

    def register_function(
        self,
        name: str,
        fn: Handler,
        *,
        capabilities: Optional[List[str]] = None,
        doc: Optional[str] = None,
    ) -> None:
        """Imperative variant of `register`."""
        self._handlers[name] = fn
        self._schema_builder.add_function(
            name=name,
            fn=fn,
            capabilities=capabilities or [],
            doc=doc or (fn.__doc__ or "").strip(),
        )

    #  Schema

    def schema_dict(self) -> dict:
        """Return the namespace schema as a plain dict (for announcement)."""
        return self._schema_builder.build()

    #  Dispatch

    async def _dispatch(
        self,
        envelope: Envelope,
        transport: BaseTransport,
    ) -> None:
        """Handle one inbound invocation envelope."""
        if envelope.invocation_type == InvocationType.BATCH:
            await self._dispatch_batch(envelope, transport)
            return

        is_cast = envelope.invocation_type == InvocationType.CAST

        fn_name = envelope.target.rsplit(".", 1)[-1]
        handler = self._handlers.get(fn_name)

        if handler is None:
            if not is_cast:
                response = _make_error(
                    envelope.id,
                    "FunctionNotFound",
                    f"no handler registered for '{envelope.target}'",
                )
                await transport.send(response)
            return

        try:
            if inspect.isasyncgenfunction(handler):
                await self._dispatch_stream(envelope, handler, transport)
            elif asyncio.iscoroutinefunction(handler):
                result = await handler(*envelope.args)
                if not is_cast:
                    await transport.send(_make_ok(envelope.id, result))
            else:
                result = handler(*envelope.args)
                if not is_cast:
                    await transport.send(_make_ok(envelope.id, result))

        except SaikuroError as exc:
            if not is_cast:
                await transport.send(
                    _make_error(envelope.id, exc.code, exc.message, exc.details)
                )
        except Exception as exc:
            logger.exception("provider error in '%s'", envelope.target)
            if not is_cast:
                await transport.send(_make_error(envelope.id, "ProviderError", str(exc)))

    async def _dispatch_batch(
        self,
        envelope: Envelope,
        transport: BaseTransport,
    ) -> None:
        """Dispatch a batch envelope: run each item and return results as a list.

        If any item fails, the whole batch fails with the item's error code plus
        ``batch_index`` and ``target`` in ``details``.
        """
        items: List[Envelope] = envelope.batch_items or []

        try:
            results: List[Any] = []

            for i, item in enumerate(items):
                sink = _ResultSink()

                # Rewrite type to CALL so the handler path runs normally.
                call_envelope = Envelope(
                    version=item.version,
                    invocation_type=InvocationType.CALL,
                    id=item.id,
                    target=item.target,
                    args=item.args,
                    capability=item.capability,
                    meta=item.meta,
                )

                await self._dispatch(call_envelope, sink)

                if sink.error is not None:
                    code = sink.error.get("code", "ProviderError")
                    message = sink.error.get("message", "unknown error")
                    await transport.send(
                        _make_error(
                            envelope.id,
                            code,
                            message,
                            {"batch_index": i, "target": item.target},
                        )
                    )
                    return

                results.append(sink.result)

            await transport.send(_make_ok(envelope.id, results))

        except Exception as exc:
            logger.exception("provider error in batch dispatch")
            await transport.send(_make_error(envelope.id, "ProviderError", str(exc)))

    async def _dispatch_stream(
        self,
        envelope: Envelope,
        handler: Handler,
        transport: BaseTransport,
    ) -> None:
        seq = 0
        try:
            async for item in handler(*envelope.args):
                response = {
                    "id": envelope.id,
                    "ok": True,
                    "result": item,
                    "seq": seq,
                }
                await transport.send(response)
                seq += 1
            # End-of-stream sentinel
            await transport.send(
                {
                    "id": envelope.id,
                    "ok": True,
                    "seq": seq,
                    "stream_control": StreamControl.END.value,
                }
            )
        except Exception as exc:
            logger.exception("stream handler error in '%s'", envelope.target)
            await transport.send(_make_error(envelope.id, "ProviderError", str(exc)))
            await transport.send(
                {
                    "id": envelope.id,
                    "ok": False,
                    "seq": seq,
                    "stream_control": StreamControl.ABORT.value,
                }
            )

    #  Server

    async def serve(self, address: str) -> None:
        """Connect to the runtime at `address` and begin serving invocations."""
        transport = make_transport(address)
        async with transport:
            await self._run_serve_loop(transport)

    async def serve_on_transport(self, transport: BaseTransport) -> None:
        """Serve invocations on an already-connected transport."""
        async with transport:
            await self._run_serve_loop(transport)

    async def _run_serve_loop(self, transport: BaseTransport) -> None:
        # Announce schema immediately after connecting.
        try:
            schema_dict = self._schema_builder.build()
            announce_env = Envelope.make_announce(schema_dict)
            await transport.send(announce_env.to_msgpack_dict())
            # Wait for the runtime's ok_empty acknowledgement.
            ack_raw = await transport.recv()
            if ack_raw is None:
                logger.warning(
                    "provider '%s': transport closed after schema announce",
                    self._namespace,
                )
                return
            try:
                ack = ResponseEnvelope.from_msgpack_dict(ack_raw)
                if not ack.ok:
                    logger.warning(
                        "provider '%s': schema announce rejected by runtime: %s",
                        self._namespace,
                        ack.error,
                    )
                else:
                    logger.debug(
                        "provider '%s': schema announce acknowledged", self._namespace
                    )
            except Exception as exc:
                logger.warning(
                    "provider '%s': could not decode schema announce ack: %s raw=%r",
                    self._namespace,
                    exc,
                    ack_raw,
                )
        except Exception as exc:
            logger.warning(
                "provider '%s': schema announce failed (continuing anyway): %s",
                self._namespace,
                exc,
            )

        logger.info("provider '%s' ready", self._namespace)
        while True:
            raw = await transport.recv()
            if raw is None:
                logger.info("provider '%s': transport closed", self._namespace)
                break
            try:
                envelope = Envelope.from_msgpack_dict(raw)
            except Exception as exc:
                logger.error(
                    "provider '%s': malformed envelope received, skipping: %s raw=%r",
                    self._namespace,
                    exc,
                    raw,
                    exc_info=True,
                )
                continue
            # Background dispatch; done-callback logs any unhandled exception.
            task = asyncio.ensure_future(self._dispatch(envelope, transport))
            task.add_done_callback(_log_task_exception)


#  Convenience decorator

# Module-level default provider (optional convenience).
_default_provider: Optional[SaikuroProvider] = None


def register_function(
    target: str,
    fn: Optional[Handler] = None,
    *,
    capabilities: Optional[List[str]] = None,
) -> Any:
    """
    Register `fn` with the module-level default provider.

    `target` must be in ``"namespace.function"`` format.
    """
    global _default_provider

    ns, _, name = target.rpartition(".")
    if not ns or not name:
        raise ValueError(f"target must be 'namespace.function', got: {target!r}")

    if _default_provider is None or _default_provider.namespace != ns:
        _default_provider = SaikuroProvider(ns)

    if fn is not None:
        _default_provider.register_function(name, fn, capabilities=capabilities)
        return fn

    # Used as a decorator without arguments.
    def decorator(f: Handler) -> Handler:
        _default_provider.register_function(name, f, capabilities=capabilities)  # type: ignore[union-attr]
        return f

    return decorator


#  Helpers


class _ResultSink(BaseTransport):
    """A no-op transport that captures the result or error from a single dispatch call."""

    def __init__(self) -> None:
        self.result: Any = None
        self.error: Optional[dict] = None

    async def connect(self) -> None:
        pass

    async def close(self) -> None:
        pass

    async def send(self, obj: dict) -> None:
        if obj.get("ok"):
            self.result = obj.get("result")
        else:
            self.error = obj.get("error")

    async def recv(self) -> Optional[dict]:
        return None


def _make_ok(inv_id: str, result: Any) -> dict:
    return {"id": inv_id, "ok": True, "result": result}


def _make_error(
    inv_id: str,
    code: str,
    message: str,
    details: Optional[dict] = None,
) -> dict:
    error: Dict[str, Any] = {"code": code, "message": message}
    if details:
        error["details"] = details
    return {"id": inv_id, "ok": False, "error": error}


def _log_task_exception(task: asyncio.Task) -> None:
    """Done-callback that logs any exception that escaped a dispatch task."""
    if task.cancelled():
        return
    exc = task.exception()
    if exc is not None:
        logger.error(
            "provider: unhandled exception in dispatch task: %s",
            exc,
            exc_info=exc,
        )
