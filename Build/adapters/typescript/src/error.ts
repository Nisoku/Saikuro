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
    this.name = new.target.name;
    this.code = payload.code;
    this.details = payload.details ?? {};
    Object.setPrototypeOf(this, new.target.prototype);
  }

  /** Construct the most specific subclass for a wire error payload. */
  static fromPayload(payload: ErrorPayload): SaikuroError {
    const klass = ERROR_MAP[payload.code] ?? SaikuroError;
    return new klass(payload);
  }
}

// Specific subclasses have constructors inherited from SaikuroError (name set via new.target.name)

export class FunctionNotFoundError extends SaikuroError {}
export class InvalidArgumentsError extends SaikuroError {}
export class CapabilityDeniedError extends SaikuroError {}
export class TransportError extends SaikuroError {}
export class SaikuroTimeoutError extends SaikuroError {}
export class ProviderError extends SaikuroError {}
export class NoProviderError extends SaikuroError {}
export class ProviderUnavailableError extends SaikuroError {}
export class ProtocolVersionError extends SaikuroError {}
export class MalformedEnvelopeError extends SaikuroError {}
export class MessageTooLargeError extends SaikuroError {}
export class BufferOverflowError extends SaikuroError {}
export class StreamClosedError extends SaikuroError {}
export class ChannelClosedError extends SaikuroError {}
export class OutOfOrderError extends SaikuroError {}

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
