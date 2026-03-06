// Saikuro:  public API surface.
//
// This is the single namespace that consumers should import/use:
//
//   using Saikuro;
//
// It re-exports all public types from the library.
//
// Core:
//   - SaikuroClient:  async client for invoking providers
//   - SaikuroProvider: register functions and serve them
//   - ITransport:  transport interface
//   - InMemoryTransport, TcpTransport, UnixSocketTransport, WebSocketTransport
//
// Streams & Channels:
//   - SaikuroStream<T>   :  server-to-client async stream
//   - SaikuroChannel<TIn, TOut> :  bidirectional channel
//
// Errors:
//   - SaikuroException (base)
//   - FunctionNotFoundException
//   - InvalidArgumentsException
//   - CapabilityDeniedException
//   - TransportException
//   - SaikuroTimeoutException
//   - ProviderException
//   - NoProviderException
//   - ProviderUnavailableException
//   - ProtocolVersionException
//   - MalformedEnvelopeException
//   - MessageTooLargeException
//   - BufferOverflowException
//   - StreamClosedException
//   - ChannelClosedException
//   - OutOfOrderException
//
// Envelopes:
//   - Envelope, ResponseEnvelope, InvocationType
//   - Envelope.MakeCall, MakeCast, MakeStreamOpen, MakeChannelOpen, MakeAnnounce, MakeResource, MakeBatch
//
// Logging:
//   - SaikuroLogger, LogLevel
//
// Types (for schema):
//   - TypeDescriptor (Primitive, List, Map, Optional, Named, Stream, Channel)
//   - T (builder: T.Bool, T.I32, T.String, etc.)
//   - RegisterOptions, ArgDescriptor
//
// Transport factory:
//   - TransportFactory.MakeTransport()

// Type aliases for convenience:  users can use "Saikuro.SaikuroClient" etc.
// The actual types live in their respective files.

using ArgDescriptor = Saikuro.ArgDescriptor;
using AsyncHandler = Saikuro.AsyncHandler;
using BufferOverflowException = Saikuro.BufferOverflowException;
using CapabilityDeniedException = Saikuro.CapabilityDeniedException;
using ChannelClosedException = Saikuro.ChannelClosedException;
using Client = Saikuro.SaikuroClient;
using ClientOptions = Saikuro.ClientOptions;
using Envelope = Saikuro.Envelope;
using FunctionNotFoundException = Saikuro.FunctionNotFoundException;
using InMemoryTransport = Saikuro.InMemoryTransport;
using InvalidArgumentsException = Saikuro.InvalidArgumentsException;
using InvocationType = Saikuro.InvocationType;
using Logger = Saikuro.SaikuroLogger;
using LogLevel = Saikuro.LogLevel;
using MalformedEnvelopeException = Saikuro.MalformedEnvelopeException;
using MessageTooLargeException = Saikuro.MessageTooLargeException;
using NoProviderException = Saikuro.NoProviderException;
using OutOfOrderException = Saikuro.OutOfOrderException;
using ProtocolVersionException = Saikuro.ProtocolVersionException;
using Provider = Saikuro.SaikuroProvider;
using ProviderException = Saikuro.ProviderException;
using ProviderUnavailableException = Saikuro.ProviderUnavailableException;
using RegisterOptions = Saikuro.RegisterOptions;
using ResourceHandle = Saikuro.ResourceHandle;
using ResponseEnvelope = Saikuro.ResponseEnvelope;
using SaikuroException = Saikuro.SaikuroException;
using SaikuroTimeoutException = Saikuro.SaikuroTimeoutException;
// Schema extraction (development mode)
using SchemaExtractor = Saikuro.Schema.SchemaExtractor;
using SchemaExtractorExtensions = Saikuro.Schema.SchemaExtractorExtensions;
using StreamClosedException = Saikuro.StreamClosedException;
using StreamHandler = Saikuro.StreamHandler;
using Transport = Saikuro.ITransport;
using TransportException = Saikuro.TransportException;
using TransportFactory = Saikuro.TransportFactory;
using TypeDescriptor = Saikuro.TypeDescriptor;
using WebSocketTransport = Saikuro.WebSocketTransport;
using XmlDocumentationParser = Saikuro.Schema.XmlDocumentationParser;
#if !WASM
using TcpTransport = Saikuro.TcpTransport;
using UnixSocketTransport = Saikuro.UnixSocketTransport;
#endif
