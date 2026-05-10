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
        var ctor = ErrorMap.TryGetValue(payload.Code, out var c) ? c : DefaultCtor;
        return ctor(payload.Code, payload.Message, payload.Details);
    }

    private static readonly Func<
        string,
        string,
        IReadOnlyDictionary<string, object?>,
        SaikuroException
    > DefaultCtor = (code, msg, det) => new SaikuroException(code, msg, det);

    private static readonly Dictionary<
        string,
        Func<string, string, IReadOnlyDictionary<string, object?>, SaikuroException>
    > ErrorMap = new()
    {
        ["NamespaceNotFound"] = (c, m, d) => new FunctionNotFoundException(c, m, d),
        ["FunctionNotFound"] = (c, m, d) => new FunctionNotFoundException(c, m, d),
        ["InvalidArguments"] = (c, m, d) => new InvalidArgumentsException(c, m, d),
        ["IncompatibleVersion"] = (c, m, d) => new ProtocolVersionException(c, m, d),
        ["MalformedEnvelope"] = (c, m, d) => new MalformedEnvelopeException(c, m, d),
        ["NoProvider"] = (c, m, d) => new NoProviderException(c, m, d),
        ["ProviderUnavailable"] = (c, m, d) => new ProviderUnavailableException(c, m, d),
        ["CapabilityDenied"] = (c, m, d) => new CapabilityDeniedException(c, m, d),
        ["CapabilityInvalid"] = (c, m, d) => new CapabilityDeniedException(c, m, d),
        ["ConnectionLost"] = (c, m, d) => new TransportException(c, m, d),
        ["MessageTooLarge"] = (c, m, d) => new MessageTooLargeException(c, m, d),
        ["Timeout"] = (c, m, d) => new SaikuroTimeoutException(c, m, d),
        ["BufferOverflow"] = (c, m, d) => new BufferOverflowException(c, m, d),
        ["ProviderError"] = (c, m, d) => new ProviderException(c, m, d),
        ["ProviderPanic"] = (c, m, d) => new ProviderException(c, m, d),
        ["StreamClosed"] = (c, m, d) => new StreamClosedException(c, m, d),
        ["ChannelClosed"] = (c, m, d) => new ChannelClosedException(c, m, d),
        ["OutOfOrder"] = (c, m, d) => new OutOfOrderException(c, m, d),
    };
}

// Subclasses — primary constructors delegate to SaikuroException(c, m, d)

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
