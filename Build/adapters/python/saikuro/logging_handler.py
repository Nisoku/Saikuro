"""
SaikuroLoggingHandler :  a Python ``logging.Handler`` that forwards structured
log records to the Saikuro runtime over a transport connection.

Instead of writing to stderr (which is lost when the process is not attached to
a terminal), this handler serialises each ``logging.LogRecord`` as a Saikuro
``log`` envelope and sends it over the provided transport.  The runtime's router
then intercepts the envelope and passes it to the configured log sink (e.g.
structured JSON to stdout, or into the runtime's own ``tracing`` subscriber).

Usage::

    import logging
    from saikuro import SaikuroClient
    from saikuro.logging_handler import SaikuroLoggingHandler

    async with SaikuroClient.connect("unix:///tmp/saikuro.sock") as client:
        handler = SaikuroLoggingHandler(client)
        logging.getLogger().addHandler(handler)
        # All subsequent log calls are forwarded to the runtime.
"""

from __future__ import annotations

import asyncio
import datetime
import logging
import traceback
from typing import Any, Dict

from .envelope import Envelope, InvocationType, LogLevel, LogRecord

# Mapping from Python logging levels to Saikuro LogLevel values.
_LEVEL_MAP: Dict[int, LogLevel] = {
    logging.DEBUG: LogLevel.DEBUG,
    logging.INFO: LogLevel.INFO,
    logging.WARNING: LogLevel.WARN,
    logging.ERROR: LogLevel.ERROR,
    logging.CRITICAL: LogLevel.ERROR,
}


def _python_level_to_saikuro(level: int) -> LogLevel:
    """Convert a Python logging level integer to a Saikuro ``LogLevel``."""
    if level <= logging.DEBUG:
        return LogLevel.TRACE
    for threshold in (logging.DEBUG, logging.INFO, logging.WARNING, logging.ERROR):
        if level <= threshold:
            return _LEVEL_MAP[threshold]
    return LogLevel.ERROR


class SaikuroLoggingHandler(logging.Handler):
    """
    A ``logging.Handler`` that forwards records to the Saikuro runtime as
    structured ``log`` envelopes.

    The handler is non-blocking: it schedules the send as an ``asyncio`` fire-
    and-forget coroutine on the running event loop.  If the event loop is not
    running (e.g. at process shutdown), the record is silently dropped :
    callers must not rely on shutdown-time log delivery.

    Args:
        client:   An open ``SaikuroClient`` whose transport is used for sending.
        level:    Minimum Python logging level to forward (default: ``NOTSET``).
    """

    def __init__(self, client: Any, level: int = logging.NOTSET) -> None:
        super().__init__(level)
        self._client = client

    def emit(self, record: logging.LogRecord) -> None:
        """Serialise ``record`` and schedule a fire-and-forget send."""
        try:
            envelope = self._make_envelope(record)
        except Exception:
            self.handleError(record)
            return

        # Schedule the send on the running loop without blocking.
        try:
            loop = asyncio.get_running_loop()
        except RuntimeError:
            # No running loop :  drop the record rather than block.
            return

        loop.create_task(self._send(envelope))

    async def _send(self, envelope: Envelope) -> None:
        try:
            await self._client._transport.send(envelope.to_msgpack_dict())
        except Exception:
            # Best-effort: if the transport is down, swallow the error rather
            # than triggering infinite recursion through the logging system.
            pass

    def _make_envelope(self, record: logging.LogRecord) -> Envelope:
        level = _python_level_to_saikuro(record.levelno)
        ts = datetime.datetime.fromtimestamp(
            record.created, tz=datetime.timezone.utc
        ).isoformat()

        fields: Dict[str, Any] = {}
        if record.exc_info:
            fields["exc"] = "".join(
                traceback.format_exception(*record.exc_info)
            ).strip()
        if record.stack_info:
            fields["stack"] = record.stack_info

        log_record = LogRecord(
            ts=ts,
            level=level,
            name=record.name,
            msg=self.format(record) if self.formatter else record.getMessage(),
            fields=fields,
        )

        return Envelope(
            version=1,
            invocation_type=InvocationType.LOG,
            id=f"log-{record.created:.6f}",
            target="$log",  # special sentinel :  the runtime never routes this
            args=[log_record.to_dict()],
        )
