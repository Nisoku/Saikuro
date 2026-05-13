"""
Saikuro error hierarchy.

Every error that crosses the Saikuro boundary maps to one of these classes,
so callers can write narrow except clauses without string-matching error
messages.
"""

from __future__ import annotations

from typing import Any, Dict, Optional


class SaikuroError(Exception):
    """Base class for all Saikuro errors."""

    def __init__(
        self,
        code: str,
        message: str,
        details: Optional[Dict[str, Any]] = None,
    ) -> None:
        super().__init__(f"[{code}] {message}")
        self.code = code
        self.message = message
        self.details: Dict[str, Any] = details or {}

    @classmethod
    def from_error_dict(cls, error_dict: Dict[str, Any]) -> "SaikuroError":
        """Construct the most specific subclass for a wire error payload."""
        code = error_dict.get("code", "Internal")
        message = error_dict.get("message", "unknown error")
        details = error_dict.get("details", {})

        mapping = {
            "NamespaceNotFound": FunctionNotFoundError,
            "FunctionNotFound": FunctionNotFoundError,
            "InvalidArguments": InvalidArgumentsError,
            "IncompatibleVersion": ProtocolVersionError,
            "MalformedEnvelope": MalformedEnvelopeError,
            "NoProvider": NoProviderError,
            "ProviderUnavailable": ProviderUnavailableError,
            "CapabilityDenied": CapabilityDeniedError,
            "CapabilityInvalid": CapabilityDeniedError,
            "ConnectionLost": TransportError,
            "MessageTooLarge": MessageTooLargeError,
            "Timeout": TimeoutError,
            "BufferOverflow": BufferOverflowError,
            "ProviderError": ProviderError,
            "ProviderPanic": ProviderError,
            "StreamClosed": StreamClosedError,
            "ChannelClosed": ChannelClosedError,
            "OutOfOrder": OutOfOrderError,
        }

        klass = mapping.get(code, SaikuroError)
        return klass(code=code, message=message, details=details)

    def __repr__(self) -> str:
        return f"{type(self).__name__}(code={self.code!r}, message={self.message!r}, details={self.details!r})"


class FunctionNotFoundError(SaikuroError):
    """The target namespace or function does not exist."""


class InvalidArgumentsError(SaikuroError):
    """One or more arguments failed type or arity validation."""


class CapabilityDeniedError(SaikuroError):
    """The caller lacks a required capability token."""


class TransportError(SaikuroError):
    """An underlying transport failure."""


class TimeoutError(SaikuroError):
    """The call timed out waiting for a response."""


class ProviderError(SaikuroError):
    """The provider returned an explicit error."""


class NoProviderError(SaikuroError):
    """No provider is registered for the target namespace."""


class ProviderUnavailableError(SaikuroError):
    """The provider is temporarily unavailable."""


class ProtocolVersionError(SaikuroError):
    """Incompatible protocol versions."""


class MalformedEnvelopeError(SaikuroError):
    """The envelope is structurally invalid."""


class MessageTooLargeError(SaikuroError):
    """A message exceeded the configured size limit."""


class BufferOverflowError(SaikuroError):
    """A stream or channel buffer overflowed."""


class StreamClosedError(SaikuroError):
    """An operation was attempted on a closed stream."""


class ChannelClosedError(SaikuroError):
    """An operation was attempted on a closed channel."""


class OutOfOrderError(SaikuroError):
    """Out-of-order sequence number detected."""
