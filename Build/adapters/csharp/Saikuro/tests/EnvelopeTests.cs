// Tests for Saikuro wire protocol envelope types

using Saikuro;

namespace Saikuro.Tests;

public class InvocationTypeTests
{
    [Theory]
    [InlineData(InvocationType.Call, "call")]
    [InlineData(InvocationType.Cast, "cast")]
    [InlineData(InvocationType.Stream, "stream")]
    [InlineData(InvocationType.Channel, "channel")]
    [InlineData(InvocationType.Batch, "batch")]
    [InlineData(InvocationType.Resource, "resource")]
    [InlineData(InvocationType.Log, "log")]
    [InlineData(InvocationType.Announce, "announce")]
    public void ToWire_ReturnsExpectedString(InvocationType type, string expected) =>
        Assert.Equal(expected, type.ToWire());

    [Theory]
    [InlineData("call", InvocationType.Call)]
    [InlineData("cast", InvocationType.Cast)]
    [InlineData("stream", InvocationType.Stream)]
    [InlineData("channel", InvocationType.Channel)]
    [InlineData("batch", InvocationType.Batch)]
    [InlineData("resource", InvocationType.Resource)]
    [InlineData("log", InvocationType.Log)]
    [InlineData("announce", InvocationType.Announce)]
    public void FromWire_ReturnsExpectedEnum(string wire, InvocationType expected) =>
        Assert.Equal(expected, InvocationTypeExt.FromWire(wire));

    [Fact]
    public void FromWire_UnknownValue_Throws() =>
        Assert.Throws<InvalidOperationException>(() => InvocationTypeExt.FromWire("unknown"));
}

public class StreamControlTests
{
    [Theory]
    [InlineData(StreamControl.End, "end")]
    [InlineData(StreamControl.Pause, "pause")]
    [InlineData(StreamControl.Resume, "resume")]
    [InlineData(StreamControl.Abort, "abort")]
    public void ToWire_ReturnsExpectedString(StreamControl sc, string expected) =>
        Assert.Equal(expected, sc.ToWire());

    [Theory]
    [InlineData("end", StreamControl.End)]
    [InlineData("pause", StreamControl.Pause)]
    [InlineData("resume", StreamControl.Resume)]
    [InlineData("abort", StreamControl.Abort)]
    public void FromWire_ReturnsExpectedEnum(string wire, StreamControl expected) =>
        Assert.Equal(expected, StreamControlExt.FromWire(wire));
}

public class ProtocolTests
{
    [Fact]
    public void Version_IsOne() => Assert.Equal(1u, Protocol.Version);
}

//  Envelope factory tests

public class EnvelopeFactoryTests
{
    [Fact]
    public void MakeCall_SetsTypeCallAndVersion()
    {
        var env = Envelope.MakeCall("math.add", [1, 2]);
        Assert.Equal(InvocationType.Call, env.Type);
        Assert.Equal(Protocol.Version, env.Version);
    }

    [Fact]
    public void MakeCall_SetsTargetAndArgs()
    {
        var env = Envelope.MakeCall("math.add", [1, 2]);
        Assert.Equal("math.add", env.Target);
        Assert.Equal(new object?[] { 1, 2 }, env.Args.ToArray());
    }

    [Fact]
    public void MakeCall_UniqueIdsAcrossCalls()
    {
        var a = Envelope.MakeCall("fn", []);
        var b = Envelope.MakeCall("fn", []);
        Assert.NotEqual(a.Id, b.Id);
    }

    [Fact]
    public void MakeCall_WithCapability_SetsCapability()
    {
        var env = Envelope.MakeCall("fn", [], "tok-123");
        Assert.Equal("tok-123", env.Capability);
    }

    [Fact]
    public void MakeCall_NoCapability_IsNull()
    {
        var env = Envelope.MakeCall("fn", []);
        Assert.Null(env.Capability);
    }

    [Fact]
    public void MakeCast_SetsTypeCast() =>
        Assert.Equal(InvocationType.Cast, Envelope.MakeCast("log.info", ["hi"]).Type);

    [Fact]
    public void MakeCast_PreservesTargetAndArgs()
    {
        var env = Envelope.MakeCast("log.info", ["hello"]);
        Assert.Equal("log.info", env.Target);
        Assert.Equal(new object?[] { "hello" }, env.Args.ToArray());
    }

    [Fact]
    public void MakeStreamOpen_SetsTypeStream() =>
        Assert.Equal(InvocationType.Stream, Envelope.MakeStreamOpen("events.sub", []).Type);

    [Fact]
    public void MakeChannelOpen_SetsTypeChannel() =>
        Assert.Equal(InvocationType.Channel, Envelope.MakeChannelOpen("chat.open", []).Type);

    [Fact]
    public void MakeResource_SetsTypeResource() =>
        Assert.Equal(InvocationType.Resource, Envelope.MakeResource("files.open", ["/tmp/x"]).Type);

    [Fact]
    public void MakeResource_WithCapability_SetsCapability()
    {
        var env = Envelope.MakeResource("files.open", [], "cap-abc");
        Assert.Equal("cap-abc", env.Capability);
    }

    [Fact]
    public void MakeBatch_SetsTypeBatch()
    {
        var items = new[] { Envelope.MakeCall("a.b", []) };
        Assert.Equal(InvocationType.Batch, Envelope.MakeBatch(items).Type);
    }

    [Fact]
    public void MakeBatch_StoresBatchItems()
    {
        var item1 = Envelope.MakeCall("svc.fn1", [1]);
        var item2 = Envelope.MakeCall("svc.fn2", [2]);
        var batch = Envelope.MakeBatch([item1, item2]);
        Assert.NotNull(batch.BatchItems);
        Assert.Equal(2, batch.BatchItems!.Count);
    }

    [Fact]
    public void MakeAnnounce_SetsTypeAnnounce()
    {
        var env = Envelope.MakeAnnounce(new Dictionary<string, object?> { ["version"] = 1 });
        Assert.Equal(InvocationType.Announce, env.Type);
    }

    [Fact]
    public void MakeAnnounce_SetsTargetToSaikuroAnnounce()
    {
        var env = Envelope.MakeAnnounce(new Dictionary<string, object?>());
        Assert.Equal("$saikuro.announce", env.Target);
    }

    [Fact]
    public void MakeAnnounce_EmbedsSchemaDictInArgs()
    {
        var schema = new Dictionary<string, object?> { ["version"] = 1 };
        var env = Envelope.MakeAnnounce(schema);
        Assert.Equal(schema, env.Args[0]);
    }
}

//  Envelope.Namespace / FunctionName helpers

public class EnvelopeHelperTests
{
    [Fact]
    public void Namespace_ExtractsPrefixBeforeLastDot()
    {
        var env = Envelope.MakeCall("math.add", []);
        Assert.Equal("math", env.Namespace);
    }

    [Fact]
    public void FunctionName_ExtractsSuffixAfterLastDot()
    {
        var env = Envelope.MakeCall("math.add", []);
        Assert.Equal("add", env.FunctionName);
    }

    [Fact]
    public void Namespace_NullWhenNoDot()
    {
        var env = Envelope.MakeCall("fn", []);
        Assert.Null(env.Namespace);
    }

    [Fact]
    public void FunctionName_NullWhenNoDot()
    {
        var env = Envelope.MakeCall("fn", []);
        Assert.Null(env.FunctionName);
    }

    [Fact]
    public void Namespace_NestedNamespace_ReturnsEverythingBeforeLastDot()
    {
        var env = Envelope.MakeCall("a.b.c", []);
        Assert.Equal("a.b", env.Namespace);
        Assert.Equal("c", env.FunctionName);
    }
}

//  Envelope round-trip serialisation

public class EnvelopeSerializationTests
{
    [Fact]
    public void CallRoundTrip_PreservesAllFields()
    {
        var original = Envelope.MakeCall("math.add", new object?[] { 3, 4 }, "tok");
        var dict = original.ToMsgpackDict();
        var restored = Envelope.FromMsgpackDict(dict);

        Assert.Equal(InvocationType.Call, restored.Type);
        Assert.Equal("math.add", restored.Target);
        Assert.Equal(original.Id, restored.Id);
        Assert.Equal("tok", restored.Capability);
    }

    [Fact]
    public void NoCapability_NotPresentInWireDict()
    {
        var env = Envelope.MakeCall("fn", []);
        var dict = env.ToMsgpackDict();
        Assert.False(dict.ContainsKey("capability"));
    }

    [Fact]
    public void Capability_PresentInWireDict()
    {
        var env = Envelope.MakeCall("fn", [], "cap");
        var dict = env.ToMsgpackDict();
        Assert.True(dict.ContainsKey("capability"));
        Assert.Equal("cap", dict["capability"]);
    }

    [Fact]
    public void StreamControl_RoundTrip()
    {
        var env = Envelope.MakeStreamOpen("s", []) with { StreamControlValue = StreamControl.End };
        var dict = env.ToMsgpackDict();
        Assert.Equal("end", dict["stream_control"]);
        var restored = Envelope.FromMsgpackDict(dict);
        Assert.Equal(StreamControl.End, restored.StreamControlValue);
    }

    [Fact]
    public void Seq_RoundTrip()
    {
        var env = Envelope.MakeCall("fn", []) with { Seq = 7 };
        var dict = env.ToMsgpackDict();
        Assert.Equal(7L, dict["seq"]);
        var restored = Envelope.FromMsgpackDict(dict);
        Assert.Equal(7ul, restored.Seq);
    }

    [Fact]
    public void BatchItems_RoundTrip()
    {
        var item1 = Envelope.MakeCall("svc.fn1", new object?[] { 1 });
        var item2 = Envelope.MakeCall("svc.fn2", new object?[] { 2 });
        var batch = Envelope.MakeBatch([item1, item2]);
        var dict = batch.ToMsgpackDict();
        var restored = Envelope.FromMsgpackDict(dict);
        Assert.NotNull(restored.BatchItems);
        Assert.Equal(2, restored.BatchItems!.Count);
        Assert.Equal("svc.fn1", restored.BatchItems[0].Target);
        Assert.Equal("svc.fn2", restored.BatchItems[1].Target);
    }

    [Fact]
    public void CastRoundTrip_TypeIsCast()
    {
        var env = Envelope.MakeCast("log.info", ["hi"]);
        var dict = env.ToMsgpackDict();
        var restored = Envelope.FromMsgpackDict(dict);
        Assert.Equal(InvocationType.Cast, restored.Type);
    }

    [Fact]
    public void AnnounceRoundTrip_TypeIsAnnounce()
    {
        var schema = new Dictionary<string, object?> { ["version"] = 1 };
        var env = Envelope.MakeAnnounce(schema);
        var dict = env.ToMsgpackDict();
        var restored = Envelope.FromMsgpackDict(dict);
        Assert.Equal(InvocationType.Announce, restored.Type);
        Assert.Equal("$saikuro.announce", restored.Target);
    }

    [Fact]
    public void WireDict_ContainsVersionTypeIdTarget()
    {
        var env = Envelope.MakeCall("x.y", []);
        var d = env.ToMsgpackDict();
        Assert.Equal(1, Convert.ToInt32(d["version"]));
        Assert.Equal("call", d["type"]);
        Assert.Equal("x.y", d["target"]);
        Assert.IsType<string>(d["id"]);
    }
}

//  ResponseEnvelope tests

public class ResponseEnvelopeTests
{
    [Fact]
    public void OkResult_ParsedCorrectly()
    {
        var resp = ResponseEnvelope.FromMsgpackDict(
            new Dictionary<string, object?>
            {
                ["id"] = "x",
                ["ok"] = true,
                ["result"] = 42L,
            }
        );
        Assert.True(resp.Ok);
        Assert.Equal(42L, resp.Result);
        Assert.Equal("x", resp.Id);
    }

    [Fact]
    public void ErrorResponse_ParsedCorrectly()
    {
        var resp = ResponseEnvelope.FromMsgpackDict(
            new Dictionary<string, object?>
            {
                ["id"] = "y",
                ["ok"] = false,
                ["error"] = new Dictionary<string, object?>
                {
                    ["code"] = "FunctionNotFound",
                    ["message"] = "not found",
                },
            }
        );
        Assert.False(resp.Ok);
        Assert.NotNull(resp.Error);
        Assert.Equal("FunctionNotFound", resp.Error!.Code);
        Assert.Equal("not found", resp.Error.Message);
    }

    [Fact]
    public void StreamEnd_IsStreamEndTrue()
    {
        var resp = ResponseEnvelope.FromMsgpackDict(
            new Dictionary<string, object?>
            {
                ["id"] = "z",
                ["ok"] = true,
                ["stream_control"] = "end",
            }
        );
        Assert.True(resp.IsStreamEnd);
    }

    [Fact]
    public void StreamAbort_IsStreamEndTrue()
    {
        var resp = ResponseEnvelope.FromMsgpackDict(
            new Dictionary<string, object?>
            {
                ["id"] = "z",
                ["ok"] = false,
                ["stream_control"] = "abort",
            }
        );
        Assert.True(resp.IsStreamEnd);
    }

    [Fact]
    public void StreamPause_IsStreamEndFalse()
    {
        var resp = ResponseEnvelope.FromMsgpackDict(
            new Dictionary<string, object?>
            {
                ["id"] = "z",
                ["ok"] = true,
                ["stream_control"] = "pause",
            }
        );
        Assert.False(resp.IsStreamEnd);
    }

    [Fact]
    public void NoStreamControl_IsStreamEndFalse()
    {
        var resp = ResponseEnvelope.FromMsgpackDict(
            new Dictionary<string, object?>
            {
                ["id"] = "z",
                ["ok"] = true,
                ["result"] = 1L,
            }
        );
        Assert.False(resp.IsStreamEnd);
    }

    [Fact]
    public void Seq_ParsedFromLong()
    {
        var resp = ResponseEnvelope.FromMsgpackDict(
            new Dictionary<string, object?>
            {
                ["id"] = "a",
                ["ok"] = true,
                ["seq"] = 5L,
            }
        );
        Assert.Equal(5ul, resp.Seq);
    }

    [Fact]
    public void NullResult_WhenResultAbsent()
    {
        var resp = ResponseEnvelope.FromMsgpackDict(
            new Dictionary<string, object?> { ["id"] = "a", ["ok"] = true }
        );
        Assert.Null(resp.Result);
    }
}

//  ErrorPayload tests

public class ErrorPayloadTests
{
    [Fact]
    public void FromMap_ParsesCodeAndMessage()
    {
        var payload = ErrorPayload.FromMap(
            new Dictionary<string, object?>
            {
                ["code"] = "FunctionNotFound",
                ["message"] = "no such function",
            }
        );
        Assert.Equal("FunctionNotFound", payload.Code);
        Assert.Equal("no such function", payload.Message);
    }

    [Fact]
    public void FromMap_DefaultCodeIsInternal_WhenAbsent()
    {
        var payload = ErrorPayload.FromMap(
            new Dictionary<string, object?> { ["message"] = "oops" }
        );
        Assert.Equal("Internal", payload.Code);
    }

    [Fact]
    public void FromMap_EmptyMessage_WhenAbsent()
    {
        var payload = ErrorPayload.FromMap(new Dictionary<string, object?> { ["code"] = "X" });
        Assert.Equal("", payload.Message);
    }

    [Fact]
    public void FromMap_ParsesDetails_WhenPresent()
    {
        var payload = ErrorPayload.FromMap(
            new Dictionary<string, object?>
            {
                ["code"] = "CapabilityDenied",
                ["message"] = "denied",
                ["details"] = new Dictionary<string, object?> { ["required"] = "admin" },
            }
        );
        Assert.True(payload.Details.ContainsKey("required"));
        Assert.Equal("admin", payload.Details["required"]);
    }

    [Fact]
    public void FromMap_EmptyDetails_WhenAbsent()
    {
        var payload = ErrorPayload.FromMap(
            new Dictionary<string, object?> { ["code"] = "X", ["message"] = "" }
        );
        Assert.Empty(payload.Details);
    }
}

//  ResourceHandle tests

public class ResourceHandleTests
{
    [Fact]
    public void FromMap_MinimalHandle_OnlyId()
    {
        var h = ResourceHandle.FromMap(new Dictionary<string, object?> { ["id"] = "res-1" });
        Assert.NotNull(h);
        Assert.Equal("res-1", h!.Id);
        Assert.Null(h.MimeType);
        Assert.Null(h.Size);
        Assert.Null(h.Uri);
    }

    [Fact]
    public void FromMap_FullHandle_ParsesAllFields()
    {
        var h = ResourceHandle.FromMap(
            new Dictionary<string, object?>
            {
                ["id"] = "res-2",
                ["mime_type"] = "image/png",
                ["size"] = 4096L,
                ["uri"] = "saikuro://res/res-2",
            }
        );
        Assert.NotNull(h);
        Assert.Equal("res-2", h!.Id);
        Assert.Equal("image/png", h.MimeType);
        Assert.Equal(4096L, h.Size);
        Assert.Equal("saikuro://res/res-2", h.Uri);
    }

    [Fact]
    public void FromMap_ReturnsNull_WhenIdMissing()
    {
        var h = ResourceHandle.FromMap(
            new Dictionary<string, object?> { ["mime_type"] = "text/plain" }
        );
        Assert.Null(h);
    }

    [Fact]
    public void FromMap_ReturnsNull_WhenIdIsNotString()
    {
        var h = ResourceHandle.FromMap(new Dictionary<string, object?> { ["id"] = 42L });
        Assert.Null(h);
    }

    [Fact]
    public void FromMap_SizeFromInt_Coerced()
    {
        var h = ResourceHandle.FromMap(
            new Dictionary<string, object?> { ["id"] = "r", ["size"] = (int)512 }
        );
        Assert.Equal(512L, h!.Size);
    }
}
