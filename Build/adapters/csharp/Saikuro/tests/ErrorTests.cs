// Tests for the Saikuro error hierarchy

using Saikuro;

namespace Saikuro.Tests;

public class SaikuroExceptionBaseTests
{
    [Fact]
    public void Code_ReturnsConstructorCode()
    {
        var ex = new SaikuroException("FunctionNotFound", "no such fn");
        Assert.Equal("FunctionNotFound", ex.Code);
    }

    [Fact]
    public void Message_IncludesCodeAndText()
    {
        var ex = new SaikuroException("FunctionNotFound", "no such fn");
        Assert.Contains("FunctionNotFound", ex.Message);
        Assert.Contains("no such fn", ex.Message);
    }

    [Fact]
    public void Details_EmptyByDefault()
    {
        var ex = new SaikuroException("X", "msg");
        Assert.Empty(ex.Details);
    }

    [Fact]
    public void Details_StoredWhenProvided()
    {
        var details = new Dictionary<string, object?> { ["key"] = "value" };
        var ex = new SaikuroException("X", "msg", details);
        Assert.Equal("value", ex.Details["key"]);
    }

    [Fact]
    public void IsException_ThrowableAndCatchable()
    {
        var thrown = Record.Exception((Action)(() => throw new SaikuroException("E", "boom")));
        Assert.IsType<SaikuroException>(thrown);
        Assert.Equal("E", ((SaikuroException)thrown!).Code);
    }
}

public class FromPayloadDispatchTests
{
    private static ErrorPayload P(
        string code,
        string msg = "test",
        IReadOnlyDictionary<string, object?>? details = null
    ) =>
        new()
        {
            Code = code,
            Message = msg,
            Details = details ?? new Dictionary<string, object?>(),
        };

    [Fact]
    public void FunctionNotFound_MapsToFunctionNotFoundException() =>
        Assert.IsType<FunctionNotFoundException>(
            SaikuroException.FromPayload(P("FunctionNotFound"))
        );

    [Fact]
    public void NamespaceNotFound_MapsToFunctionNotFoundException() =>
        Assert.IsType<FunctionNotFoundException>(
            SaikuroException.FromPayload(P("NamespaceNotFound"))
        );

    [Fact]
    public void InvalidArguments_MapsToInvalidArgumentsException() =>
        Assert.IsType<InvalidArgumentsException>(
            SaikuroException.FromPayload(P("InvalidArguments"))
        );

    [Fact]
    public void CapabilityDenied_MapsToCapabilityDeniedException() =>
        Assert.IsType<CapabilityDeniedException>(
            SaikuroException.FromPayload(P("CapabilityDenied"))
        );

    [Fact]
    public void CapabilityInvalid_MapsToCapabilityDeniedException() =>
        Assert.IsType<CapabilityDeniedException>(
            SaikuroException.FromPayload(P("CapabilityInvalid"))
        );

    [Fact]
    public void ConnectionLost_MapsToTransportException() =>
        Assert.IsType<TransportException>(SaikuroException.FromPayload(P("ConnectionLost")));

    [Fact]
    public void Timeout_MapsToSaikuroTimeoutException() =>
        Assert.IsType<SaikuroTimeoutException>(SaikuroException.FromPayload(P("Timeout")));

    [Fact]
    public void NoProvider_MapsToNoProviderException() =>
        Assert.IsType<NoProviderException>(SaikuroException.FromPayload(P("NoProvider")));

    [Fact]
    public void ProviderUnavailable_MapsToProviderUnavailableException() =>
        Assert.IsType<ProviderUnavailableException>(
            SaikuroException.FromPayload(P("ProviderUnavailable"))
        );

    [Fact]
    public void IncompatibleVersion_MapsToProtocolVersionException() =>
        Assert.IsType<ProtocolVersionException>(
            SaikuroException.FromPayload(P("IncompatibleVersion"))
        );

    [Fact]
    public void MalformedEnvelope_MapsToMalformedEnvelopeException() =>
        Assert.IsType<MalformedEnvelopeException>(
            SaikuroException.FromPayload(P("MalformedEnvelope"))
        );

    [Fact]
    public void MessageTooLarge_MapsToMessageTooLargeException() =>
        Assert.IsType<MessageTooLargeException>(SaikuroException.FromPayload(P("MessageTooLarge")));

    [Fact]
    public void BufferOverflow_MapsToBufferOverflowException() =>
        Assert.IsType<BufferOverflowException>(SaikuroException.FromPayload(P("BufferOverflow")));

    [Fact]
    public void ProviderError_MapsToProviderException() =>
        Assert.IsType<ProviderException>(SaikuroException.FromPayload(P("ProviderError")));

    [Fact]
    public void ProviderPanic_MapsToProviderException() =>
        Assert.IsType<ProviderException>(SaikuroException.FromPayload(P("ProviderPanic")));

    [Fact]
    public void StreamClosed_MapsToStreamClosedException() =>
        Assert.IsType<StreamClosedException>(SaikuroException.FromPayload(P("StreamClosed")));

    [Fact]
    public void ChannelClosed_MapsToChannelClosedException() =>
        Assert.IsType<ChannelClosedException>(SaikuroException.FromPayload(P("ChannelClosed")));

    [Fact]
    public void OutOfOrder_MapsToOutOfOrderException() =>
        Assert.IsType<OutOfOrderException>(SaikuroException.FromPayload(P("OutOfOrder")));

    [Fact]
    public void UnknownCode_FallsBackToBaseSaikuroException()
    {
        var ex = SaikuroException.FromPayload(P("SomeNewCode"));
        Assert.IsType<SaikuroException>(ex);
        Assert.Equal("SomeNewCode", ex.Code);
    }

    [Fact]
    public void FromPayload_PreservesMessageText()
    {
        var ex = SaikuroException.FromPayload(P("FunctionNotFound", "my custom message"));
        Assert.Contains("my custom message", ex.Message);
    }

    [Fact]
    public void FromPayload_PreservesDetails()
    {
        var details = new Dictionary<string, object?> { ["required"] = "admin" };
        var ex = SaikuroException.FromPayload(
            new ErrorPayload
            {
                Code = "CapabilityDenied",
                Message = "denied",
                Details = details,
            }
        );
        Assert.Equal("admin", ex.Details["required"]);
    }
}

public class ErrorSubclassTests
{
    // All subclasses must be catchable as SaikuroException.
    [Fact]
    public void AllSubclassesAreSaikuroException()
    {
        SaikuroException[] exceptions =
        [
            new FunctionNotFoundException(
                "FunctionNotFound",
                "a",
                new Dictionary<string, object?>()
            ),
            new InvalidArgumentsException(
                "InvalidArguments",
                "a",
                new Dictionary<string, object?>()
            ),
            new CapabilityDeniedException(
                "CapabilityDenied",
                "a",
                new Dictionary<string, object?>()
            ),
            new TransportException("ConnectionLost", "a", new Dictionary<string, object?>()),
            new SaikuroTimeoutException("Timeout", "a", new Dictionary<string, object?>()),
            new ProviderException("ProviderError", "a", new Dictionary<string, object?>()),
            new NoProviderException("NoProvider", "a", new Dictionary<string, object?>()),
            new ProviderUnavailableException(
                "ProviderUnavailable",
                "a",
                new Dictionary<string, object?>()
            ),
            new ProtocolVersionException(
                "IncompatibleVersion",
                "a",
                new Dictionary<string, object?>()
            ),
            new MalformedEnvelopeException(
                "MalformedEnvelope",
                "a",
                new Dictionary<string, object?>()
            ),
            new MessageTooLargeException("MessageTooLarge", "a", new Dictionary<string, object?>()),
            new BufferOverflowException("BufferOverflow", "a", new Dictionary<string, object?>()),
            new StreamClosedException("StreamClosed", "a", new Dictionary<string, object?>()),
            new ChannelClosedException("ChannelClosed", "a", new Dictionary<string, object?>()),
            new OutOfOrderException("OutOfOrder", "a", new Dictionary<string, object?>()),
        ];

        foreach (var ex in exceptions)
            Assert.IsAssignableFrom<SaikuroException>(ex);
    }

    [Fact]
    public void AllSubclassesAreExceptions()
    {
        SaikuroException ex = new FunctionNotFoundException(
            "FunctionNotFound",
            "msg",
            new Dictionary<string, object?>()
        );
        Assert.IsAssignableFrom<Exception>(ex);
    }
}
