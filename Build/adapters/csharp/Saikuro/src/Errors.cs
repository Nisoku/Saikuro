// Saikuro error hierarchy.
//
// Every wire error code maps to a distinct Exception subclass so callers can
// write narrow catch clauses without string-matching error messages.

namespace Saikuro;

// Base

/// <summary>Base exception for all Saikuro errors.</summary>
public class SaikuroException : Exception
{
    public string Code { get; }
    public IReadOnlyDictionary<string, object?> Details { get; }

    public SaikuroException(
        string code,
        string message,
        IReadOnlyDictionary<string, object?>? details = null
    )
        : base($"[{code}] {message}")
    {
        Code = code;
        Details = details ?? new Dictionary<string, object?>();
    }

    /// <summary>Construct the most specific subclass for a wire error payload.</summary>
    public static SaikuroException FromPayload(ErrorPayload payload)
    {
        if (ErrorTypeMap.TryGetValue(payload.Code, out var exceptionType))
        {
            return (SaikuroException)Activator.CreateInstance(
                exceptionType,
                payload.Code,
                payload.Message,
                payload.Details
            )!;
        }
        return new SaikuroException(payload.Code, payload.Message, payload.Details);
    }

    private static readonly Func<
        string,
        string,
        IReadOnlyDictionary<string, object?>,
        SaikuroException
    > DefaultCtor = (code, msg, det) => new SaikuroException(code, msg, det);

    private static readonly Dictionary<string, Type> ErrorTypeMap = new()
    {
        ["NamespaceNotFound"] = typeof(FunctionNotFoundException),
        ["FunctionNotFound"] = typeof(FunctionNotFoundException),
        ["InvalidArguments"] = typeof(InvalidArgumentsException),
        ["IncompatibleVersion"] = typeof(ProtocolVersionException),
        ["MalformedEnvelope"] = typeof(MalformedEnvelopeException),
        ["NoProvider"] = typeof(NoProviderException),
        ["ProviderUnavailable"] = typeof(ProviderUnavailableException),
        ["CapabilityDenied"] = typeof(CapabilityDeniedException),
        ["CapabilityInvalid"] = typeof(CapabilityDeniedException),
        ["ConnectionLost"] = typeof(TransportException),
        ["MessageTooLarge"] = typeof(MessageTooLargeException),
        ["Timeout"] = typeof(SaikuroTimeoutException),
        ["BufferOverflow"] = typeof(BufferOverflowException),
        ["ProviderError"] = typeof(ProviderException),
        ["ProviderPanic"] = typeof(ProviderException),
        ["StreamClosed"] = typeof(StreamClosedException),
        ["ChannelClosed"] = typeof(ChannelClosedException),
        ["OutOfOrder"] = typeof(OutOfOrderException),
    };
}

// Subclasses as primary constructors delegate to SaikuroException(c, m, d)

public sealed class FunctionNotFoundException(string c, string m, IReadOnlyDictionary<string, object?> d) : SaikuroException(c, m, d);
public sealed class InvalidArgumentsException(string c, string m, IReadOnlyDictionary<string, object?> d) : SaikuroException(c, m, d);
public sealed class CapabilityDeniedException(string c, string m, IReadOnlyDictionary<string, object?> d) : SaikuroException(c, m, d);
public sealed class TransportException(string c, string m, IReadOnlyDictionary<string, object?> d) : SaikuroException(c, m, d);
public sealed class SaikuroTimeoutException(string c, string m, IReadOnlyDictionary<string, object?> d) : SaikuroException(c, m, d);
public sealed class ProviderException(string c, string m, IReadOnlyDictionary<string, object?> d) : SaikuroException(c, m, d);
public sealed class NoProviderException(string c, string m, IReadOnlyDictionary<string, object?> d) : SaikuroException(c, m, d);
public sealed class ProviderUnavailableException(string c, string m, IReadOnlyDictionary<string, object?> d) : SaikuroException(c, m, d);
public sealed class ProtocolVersionException(string c, string m, IReadOnlyDictionary<string, object?> d) : SaikuroException(c, m, d);
public sealed class MalformedEnvelopeException(string c, string m, IReadOnlyDictionary<string, object?> d) : SaikuroException(c, m, d);
public sealed class MessageTooLargeException(string c, string m, IReadOnlyDictionary<string, object?> d) : SaikuroException(c, m, d);
public sealed class BufferOverflowException(string c, string m, IReadOnlyDictionary<string, object?> d) : SaikuroException(c, m, d);
public sealed class StreamClosedException(string c, string m, IReadOnlyDictionary<string, object?> d) : SaikuroException(c, m, d);
public sealed class ChannelClosedException(string c, string m, IReadOnlyDictionary<string, object?> d) : SaikuroException(c, m, d);
public sealed class OutOfOrderException(string c, string m, IReadOnlyDictionary<string, object?> d) : SaikuroException(c, m, d);
