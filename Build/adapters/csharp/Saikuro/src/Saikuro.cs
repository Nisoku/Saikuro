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
//   - WireKey: protocol constants
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
using WireKey = Saikuro.WireKey;
using XmlDocumentationParser = Saikuro.Schema.XmlDocumentationParser;
#if !WASM
using TcpTransport = Saikuro.TcpTransport;
using UnixSocketTransport = Saikuro.UnixSocketTransport;
#endif

// Compile-time verification: if any aliased type is renamed, these references
// will fail to compile, alerting the maintainer that the alias needs updating.
// This runs only on Debug builds to avoid any JIT overhead.
#if DEBUG
internal static class _AliasVerifier
{
    internal static void Verify()
    {
        _ = typeof(ArgDescriptor);
        _ = typeof(AsyncHandler);
        _ = typeof(BufferOverflowException);
        _ = typeof(CapabilityDeniedException);
        _ = typeof(ChannelClosedException);
        _ = typeof(ClientOptions);
        _ = typeof(Envelope);
        _ = typeof(FunctionNotFoundException);
        _ = typeof(InMemoryTransport);
        _ = typeof(InvalidArgumentsException);
        _ = typeof(InvocationType);
        _ = typeof(MalformedEnvelopeException);
        _ = typeof(MessageTooLargeException);
        _ = typeof(NoProviderException);
        _ = typeof(OutOfOrderException);
        _ = typeof(ProtocolVersionException);
        _ = typeof(ProviderException);
        _ = typeof(ProviderUnavailableException);
        _ = typeof(RegisterOptions);
        _ = typeof(ResourceHandle);
        _ = typeof(ResponseEnvelope);
        _ = typeof(SaikuroTimeoutException);
        _ = typeof(SchemaExtractor);
        _ = typeof(SchemaExtractorExtensions);
        _ = typeof(StreamClosedException);
        _ = typeof(StreamHandler);
        _ = typeof(TransportException);
        _ = typeof(TransportFactory);
        _ = typeof(TypeDescriptor);
        _ = typeof(WebSocketTransport);
        _ = typeof(XmlDocumentationParser);
#if !WASM
        _ = typeof(TcpTransport);
        _ = typeof(UnixSocketTransport);
#endif
    }
}
#endif
