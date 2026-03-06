/**
 * Saikuro TypeScript adapter: public API surface.
 *
 * Import from here:
 *
 *   import {
 *     SaikuroClient,
 *     SaikuroProvider,
 *     SaikuroStream,
 *     SaikuroChannel,
 *     SaikuroError,
 *     makeTransport,
 *   } from "@nisoku/saikuro";
 */

// Envelope / protocol types

export type {
  Envelope,
  ResponseEnvelope,
  ErrorPayload,
  ErrorCode,
  InvocationType,
  StreamControl,
  SaikuroSchema,
  NamespaceSchema,
  FunctionSchema as EnvelopeFunctionSchema,
  ArgumentDescriptor,
  ResourceHandle,
} from "./envelope";

export {
  PROTOCOL_VERSION,
  makeCallEnvelope,
  makeCastEnvelope,
  makeStreamOpenEnvelope,
  makeChannelOpenEnvelope,
  makeAnnounceEnvelope,
  makeResourceEnvelope,
  makeBatchEnvelope,
  decodeResourceHandle,
  generateId,
} from "./envelope";

// Errors

export {
  SaikuroError,
  FunctionNotFoundError,
  InvalidArgumentsError,
  CapabilityDeniedError,
  TransportError,
  SaikuroTimeoutError,
  ProviderError,
  NoProviderError,
  ProviderUnavailableError,
  ProtocolVersionError,
  MalformedEnvelopeError,
  MessageTooLargeError,
  BufferOverflowError,
  StreamClosedError,
  ChannelClosedError,
  OutOfOrderError,
} from "./error";

// Transport

export type { Transport } from "./transport";

export {
  InMemoryTransport,
  WebSocketTransport,
  NodeStreamTransport,
  makeTransport,
} from "./transport";

// Client

export { SaikuroClient, SaikuroStream, SaikuroChannel } from "./client";
export type { ClientOptions } from "./client";

// Provider

export { SaikuroProvider, t } from "./provider";
export type {
  Handler,
  StreamHandler,
  AnyHandler,
  FunctionSchema,
  ArgDescriptor,
  TypeDescriptor,
} from "./provider";

// Logging

export {
  getLogger,
  setLogSink,
  setLogLevel,
  resetLogSink,
  createTransportSink,
} from "./logger";
export type { Logger, LogLevel, LogRecord, TransportLike } from "./logger";

// Schema Extraction (Development Mode)

export { SchemaExtractor, extractSchema } from "./schema_extractor";
export type {
  ExtractedArg,
  ExtractedFunction,
  TypeDescriptor as ExtractedTypeDescriptor,
} from "./schema_extractor";
