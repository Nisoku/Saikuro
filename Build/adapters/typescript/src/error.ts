/**
 * Saikuro error classes.
 *
 * Every wire error code maps to a distinct Error subclass so callers can
 * write narrow `instanceof` checks without parsing error strings.
 */

import type { ErrorCode, ErrorPayload } from "./envelope";

// Base

export class SaikuroError extends Error {
  readonly code: ErrorCode;
  readonly details: Readonly<Record<string, unknown>>;

  constructor(payload: ErrorPayload) {
    super(`[${payload.code}] ${payload.message}`);
    this.name = "SaikuroError";
    this.code = payload.code;
    this.details = payload.details ?? {};
    // Restore prototype chain (needed when compiling to ES5).
    Object.setPrototypeOf(this, new.target.prototype);
  }

  /** Construct the most specific subclass for a wire error payload. */
  static fromPayload(payload: ErrorPayload): SaikuroError {
    const klass = ERROR_MAP[payload.code] ?? SaikuroError;
    return new klass(payload);
  }
}

// Specific subclasses

export class FunctionNotFoundError extends SaikuroError {
  constructor(p: ErrorPayload) {
    super(p);
    this.name = "FunctionNotFoundError";
  }
}

export class InvalidArgumentsError extends SaikuroError {
  constructor(p: ErrorPayload) {
    super(p);
    this.name = "InvalidArgumentsError";
  }
}

export class CapabilityDeniedError extends SaikuroError {
  constructor(p: ErrorPayload) {
    super(p);
    this.name = "CapabilityDeniedError";
  }
}

export class TransportError extends SaikuroError {
  constructor(p: ErrorPayload) {
    super(p);
    this.name = "TransportError";
  }
}

export class SaikuroTimeoutError extends SaikuroError {
  constructor(p: ErrorPayload) {
    super(p);
    this.name = "SaikuroTimeoutError";
  }
}

export class ProviderError extends SaikuroError {
  constructor(p: ErrorPayload) {
    super(p);
    this.name = "ProviderError";
  }
}

export class NoProviderError extends SaikuroError {
  constructor(p: ErrorPayload) {
    super(p);
    this.name = "NoProviderError";
  }
}

export class ProviderUnavailableError extends SaikuroError {
  constructor(p: ErrorPayload) {
    super(p);
    this.name = "ProviderUnavailableError";
  }
}

export class ProtocolVersionError extends SaikuroError {
  constructor(p: ErrorPayload) {
    super(p);
    this.name = "ProtocolVersionError";
  }
}

export class MalformedEnvelopeError extends SaikuroError {
  constructor(p: ErrorPayload) {
    super(p);
    this.name = "MalformedEnvelopeError";
  }
}

export class MessageTooLargeError extends SaikuroError {
  constructor(p: ErrorPayload) {
    super(p);
    this.name = "MessageTooLargeError";
  }
}

export class BufferOverflowError extends SaikuroError {
  constructor(p: ErrorPayload) {
    super(p);
    this.name = "BufferOverflowError";
  }
}

export class StreamClosedError extends SaikuroError {
  constructor(p: ErrorPayload) {
    super(p);
    this.name = "StreamClosedError";
  }
}

export class ChannelClosedError extends SaikuroError {
  constructor(p: ErrorPayload) {
    super(p);
    this.name = "ChannelClosedError";
  }
}

export class OutOfOrderError extends SaikuroError {
  constructor(p: ErrorPayload) {
    super(p);
    this.name = "OutOfOrderError";
  }
}

// Mapping

type ErrorConstructor = new (p: ErrorPayload) => SaikuroError;

const ERROR_MAP: Partial<Record<ErrorCode, ErrorConstructor>> = {
  NamespaceNotFound: FunctionNotFoundError,
  FunctionNotFound: FunctionNotFoundError,
  InvalidArguments: InvalidArgumentsError,
  IncompatibleVersion: ProtocolVersionError,
  MalformedEnvelope: MalformedEnvelopeError,
  NoProvider: NoProviderError,
  ProviderUnavailable: ProviderUnavailableError,
  CapabilityDenied: CapabilityDeniedError,
  CapabilityInvalid: CapabilityDeniedError,
  ConnectionLost: TransportError,
  MessageTooLarge: MessageTooLargeError,
  Timeout: SaikuroTimeoutError,
  BufferOverflow: BufferOverflowError,
  ProviderError: ProviderError,
  ProviderPanic: ProviderError,
  StreamClosed: StreamClosedError,
  ChannelClosed: ChannelClosedError,
  OutOfOrder: OutOfOrderError,
};
