// Tests for SaikuroProvider

using Saikuro;

namespace Saikuro.Tests;

//  Helpers

internal static class ProviderTestHelpers
{
    public static Envelope MakeEnvelope(
        InvocationType type,
        string target,
        IReadOnlyList<object?> args,
        string id = "test-id"
    ) =>
        new()
        {
            Version = Protocol.Version,
            Type = type,
            Id = id,
            Target = target,
            Args = args,
        };

    public static async Task<List<Dictionary<string, object?>>> CollectN(
        InMemoryTransport transport,
        int n,
        TimeSpan? timeout = null
    )
    {
        using var cts = new CancellationTokenSource(timeout ?? TimeSpan.FromSeconds(5));
        var results = new List<Dictionary<string, object?>>();
        for (int i = 0; i < n; i++)
        {
            var msg = await transport.RecvAsync(cts.Token);
            if (msg is null)
                break;
            results.Add(msg);
        }
        return results;
    }
}

//  Namespace / Register

public class ProviderNamespaceTests
{
    [Fact]
    public void Namespace_ReturnsConstructorValue()
    {
        var p = new SaikuroProvider("math");
        Assert.Equal("math", p.Namespace);
    }
}

public class ProviderRegisterTests
{
    [Fact]
    public void Register_SyncHandler_Fluent_ReturnsSelf()
    {
        var p = new SaikuroProvider("test");
        var result = p.Register("fn", (IReadOnlyList<object?> _) => (object?)null);
        Assert.Same(p, result);
    }

    [Fact]
    public void Register_AsyncHandler_Fluent_ReturnsSelf()
    {
        var p = new SaikuroProvider("test");
        var result = p.Register("fn", (IReadOnlyList<object?> _) => Task.FromResult<object?>(null));
        Assert.Same(p, result);
    }

    [Fact]
    public void Register_StreamHandler_Fluent_ReturnsSelf()
    {
        var p = new SaikuroProvider("test");
        var result = p.Register(
            "fn",
            (Func<IReadOnlyList<object?>, IAsyncEnumerable<object?>>)(_ => EmptyStream())
        );
        Assert.Same(p, result);
    }

    [Fact]
    public async Task Register_SyncHandler_DispatchableViaCall()
    {
        var (a, b) = InMemoryTransport.Pair();
        var p = new SaikuroProvider("test");
        p.Register(
            "double",
            (IReadOnlyList<object?> args) => (object?)(Convert.ToInt64(args[0]) * 2)
        );

        await p.DispatchAsync(
            ProviderTestHelpers.MakeEnvelope(InvocationType.Call, "test.double", [5L]),
            a
        );
        var resp = await b.RecvAsync();
        Assert.NotNull(resp);
        Assert.True((bool)resp!["ok"]!);
        Assert.Equal(10L, resp["result"]);
    }

    private static async IAsyncEnumerable<object?> EmptyStream()
    {
        await Task.CompletedTask;
        yield break;
    }
}

//  DispatchAsync: sync handler

public class ProviderDispatchSyncTests
{
    [Fact]
    public async Task Dispatch_ReturnsCorrectResult()
    {
        var (a, b) = InMemoryTransport.Pair();
        var p = new SaikuroProvider("math");
        p.Register(
            "add",
            (IReadOnlyList<object?> args) =>
                (object?)(Convert.ToInt64(args[0]) + Convert.ToInt64(args[1]))
        );

        await p.DispatchAsync(
            ProviderTestHelpers.MakeEnvelope(InvocationType.Call, "math.add", [3L, 4L]),
            a
        );
        var resp = await b.RecvAsync();

        Assert.NotNull(resp);
        Assert.Equal("test-id", resp!["id"]);
        Assert.True((bool)resp["ok"]!);
        Assert.Equal(7L, resp["result"]);
    }

    [Fact]
    public async Task Dispatch_UsesLastSegmentOfTarget()
    {
        var (a, b) = InMemoryTransport.Pair();
        var p = new SaikuroProvider("math");
        p.Register(
            "mul",
            (IReadOnlyList<object?> args) =>
                (object?)(Convert.ToInt64(args[0]) * Convert.ToInt64(args[1]))
        );

        await p.DispatchAsync(
            ProviderTestHelpers.MakeEnvelope(InvocationType.Call, "math.mul", [3L, 5L]),
            a
        );
        var resp = await b.RecvAsync();
        Assert.Equal(15L, resp!["result"]);
    }

    [Fact]
    public async Task Dispatch_NullResult()
    {
        var (a, b) = InMemoryTransport.Pair();
        var p = new SaikuroProvider("test");
        p.Register("nil", (IReadOnlyList<object?> _) => (object?)null);

        await p.DispatchAsync(
            ProviderTestHelpers.MakeEnvelope(InvocationType.Call, "test.nil", []),
            a
        );
        var resp = await b.RecvAsync();
        Assert.NotNull(resp);
        Assert.True((bool)resp!["ok"]!);
        Assert.Null(resp["result"]);
    }
}

//  DispatchAsync: async handler

public class ProviderDispatchAsyncTests
{
    [Fact]
    public async Task Dispatch_AwaitsAsyncHandler()
    {
        var (a, b) = InMemoryTransport.Pair();
        var p = new SaikuroProvider("math");
        p.Register(
            "async_add",
            (Func<IReadOnlyList<object?>, Task<object?>>)(
                args =>
                    Task.FromResult<object?>(Convert.ToInt64(args[0]) + Convert.ToInt64(args[1]))
            )
        );

        await p.DispatchAsync(
            ProviderTestHelpers.MakeEnvelope(InvocationType.Call, "math.async_add", [10L, 20L]),
            a
        );
        var resp = await b.RecvAsync();
        Assert.Equal(30L, resp!["result"]);
    }
}

//  DispatchAsync: streaming handler

public class ProviderDispatchStreamTests
{
    [Fact]
    public async Task Dispatch_Stream_YieldsItemsWithSeqThenEnd()
    {
        var (a, b) = InMemoryTransport.Pair();
        var p = new SaikuroProvider("test");
        p.Register(
            "count",
            (Func<IReadOnlyList<object?>, IAsyncEnumerable<object?>>)(_ => CountAsync())
        );

        await p.DispatchAsync(
            ProviderTestHelpers.MakeEnvelope(InvocationType.Stream, "test.count", []),
            a
        );
        var responses = await ProviderTestHelpers.CollectN(b, 4);

        Assert.Equal(4, responses.Count);
        Assert.Equal("a", responses[0]["result"]);
        Assert.Equal(0L, responses[0]["seq"]);
        Assert.Equal("b", responses[1]["result"]);
        Assert.Equal(1L, responses[1]["seq"]);
        Assert.Equal("c", responses[2]["result"]);
        Assert.Equal(2L, responses[2]["seq"]);
        Assert.Equal("end", responses[3]["stream_control"]);
        Assert.True((bool)responses[3]["ok"]!);
    }

    [Fact]
    public async Task Dispatch_EmptyGenerator_SendsOnlyEnd()
    {
        var (a, b) = InMemoryTransport.Pair();
        var p = new SaikuroProvider("test");
        p.Register(
            "empty",
            (Func<IReadOnlyList<object?>, IAsyncEnumerable<object?>>)(_ => EmptyAsync())
        );

        await p.DispatchAsync(
            ProviderTestHelpers.MakeEnvelope(InvocationType.Stream, "test.empty", []),
            a
        );
        var responses = await ProviderTestHelpers.CollectN(b, 1);

        Assert.Single(responses);
        Assert.Equal("end", responses[0]["stream_control"]);
    }

    [Fact]
    public async Task Dispatch_MidStreamException_SendsErrorThenAbort()
    {
        var (a, b) = InMemoryTransport.Pair();
        var p = new SaikuroProvider("test");
        p.Register(
            "fail_stream",
            (Func<IReadOnlyList<object?>, IAsyncEnumerable<object?>>)(_ => FailStreamAsync())
        );

        await p.DispatchAsync(
            ProviderTestHelpers.MakeEnvelope(InvocationType.Stream, "test.fail_stream", []),
            a
        );
        var responses = await ProviderTestHelpers.CollectN(b, 3);

        Assert.Equal(3, responses.Count);
        // First item.
        Assert.True((bool)responses[0]["ok"]!);
        Assert.Equal(42L, responses[0]["result"]);
        // Error frame.
        Assert.False((bool)responses[1]["ok"]!);
        var errMap = (Dictionary<string, object?>)responses[1]["error"]!;
        Assert.Contains("mid-stream failure", (string)errMap["message"]!);
        // Abort sentinel.
        Assert.Equal("abort", responses[2]["stream_control"]);
    }

    private static async IAsyncEnumerable<object?> CountAsync()
    {
        yield return "a";
        yield return "b";
        yield return "c";
        await Task.CompletedTask;
    }

    private static async IAsyncEnumerable<object?> EmptyAsync()
    {
        await Task.CompletedTask;
        yield break;
    }

    private static async IAsyncEnumerable<object?> FailStreamAsync()
    {
        yield return 42L;
        await Task.CompletedTask;
        throw new InvalidOperationException("mid-stream failure");
    }
}

//  DispatchAsync: error cases

public class ProviderDispatchErrorTests
{
    [Fact]
    public async Task Dispatch_FunctionNotFound_ReturnsErrorResponse()
    {
        var (a, b) = InMemoryTransport.Pair();
        var p = new SaikuroProvider("test");

        await p.DispatchAsync(
            ProviderTestHelpers.MakeEnvelope(InvocationType.Call, "test.missing", []),
            a
        );
        var resp = await b.RecvAsync();

        Assert.NotNull(resp);
        Assert.False((bool)resp!["ok"]!);
        var err = (Dictionary<string, object?>)resp["error"]!;
        Assert.Equal("FunctionNotFound", err["code"]);
    }

    [Fact]
    public async Task Dispatch_PlainException_BecomesProviderError()
    {
        var (a, b) = InMemoryTransport.Pair();
        var p = new SaikuroProvider("test");
        p.Register(
            "boom",
            (Func<IReadOnlyList<object?>, object?>)(
                _ => throw new InvalidOperationException("exploded")
            )
        );

        await p.DispatchAsync(
            ProviderTestHelpers.MakeEnvelope(InvocationType.Call, "test.boom", []),
            a
        );
        var resp = await b.RecvAsync();

        Assert.NotNull(resp);
        Assert.False((bool)resp!["ok"]!);
        var err = (Dictionary<string, object?>)resp["error"]!;
        Assert.Equal("ProviderError", err["code"]);
        Assert.Contains("exploded", (string)err["message"]!);
    }

    [Fact]
    public async Task Dispatch_SaikuroException_PreservesCodeAndMessage()
    {
        var (a, b) = InMemoryTransport.Pair();
        var p = new SaikuroProvider("test");
        p.Register(
            "deny",
            (Func<IReadOnlyList<object?>, object?>)(
                _ =>
                    throw new CapabilityDeniedException(
                        "CapabilityDenied",
                        "access denied",
                        new Dictionary<string, object?>()
                    )
            )
        );

        await p.DispatchAsync(
            ProviderTestHelpers.MakeEnvelope(InvocationType.Call, "test.deny", []),
            a
        );
        var resp = await b.RecvAsync();

        Assert.NotNull(resp);
        Assert.False((bool)resp!["ok"]!);
        var err = (Dictionary<string, object?>)resp["error"]!;
        Assert.Equal("CapabilityDenied", err["code"]);
        Assert.Contains("access denied", (string)err["message"]!);
    }
}

//  SchemaDict tests

public class ProviderSchemaDictTests
{
    [Fact]
    public void SchemaDict_VersionIsOne()
    {
        var p = new SaikuroProvider("math");
        Assert.Equal(1, p.SchemaDict()["version"]);
    }

    [Fact]
    public void SchemaDict_NamespacePresent()
    {
        var p = new SaikuroProvider("math");
        p.Register("add", (IReadOnlyList<object?> _) => (object?)null);
        var ns = (Dictionary<string, object?>)p.SchemaDict()["namespaces"]!;
        Assert.True(ns.ContainsKey("math"));
    }

    [Fact]
    public void SchemaDict_RegisteredFunctionsPresent()
    {
        var p = new SaikuroProvider("math");
        p.Register("add", (IReadOnlyList<object?> _) => (object?)null);
        p.Register("sub", (IReadOnlyList<object?> _) => (object?)null);

        var ns = (Dictionary<string, object?>)p.SchemaDict()["namespaces"]!;
        var fns =
            (Dictionary<string, object?>)((Dictionary<string, object?>)ns["math"]!)["functions"]!;
        Assert.True(fns.ContainsKey("add"));
        Assert.True(fns.ContainsKey("sub"));
    }

    [Fact]
    public void SchemaDict_EmptyFunctions_WhenNothingRegistered()
    {
        var p = new SaikuroProvider("empty");
        var ns = (Dictionary<string, object?>)p.SchemaDict()["namespaces"]!;
        var fns =
            (Dictionary<string, object?>)((Dictionary<string, object?>)ns["empty"]!)["functions"]!;
        Assert.Empty(fns);
    }

    [Fact]
    public void SchemaDict_DocStoredWhenProvided()
    {
        var p = new SaikuroProvider("math");
        p.Register(
            "add",
            (IReadOnlyList<object?> _) => (object?)null,
            new RegisterOptions { Doc = "adds two numbers" }
        );

        var ns = (Dictionary<string, object?>)p.SchemaDict()["namespaces"]!;
        var fns =
            (Dictionary<string, object?>)((Dictionary<string, object?>)ns["math"]!)["functions"]!;
        var fn = (Dictionary<string, object?>)fns["add"]!;
        Assert.Equal("adds two numbers", fn["doc"]);
    }

    [Fact]
    public void SchemaDict_CapabilitiesStored()
    {
        var p = new SaikuroProvider("math");
        p.Register(
            "secret",
            (IReadOnlyList<object?> _) => (object?)null,
            new RegisterOptions { Capabilities = new[] { "admin" } }
        );

        var ns = (Dictionary<string, object?>)p.SchemaDict()["namespaces"]!;
        var fns =
            (Dictionary<string, object?>)((Dictionary<string, object?>)ns["math"]!)["functions"]!;
        var fn = (Dictionary<string, object?>)fns["secret"]!;
        var caps = (IReadOnlyList<object?>)fn["capabilities"]!;
        Assert.Contains("admin", caps.Cast<string>());
    }

    [Fact]
    public void SchemaDict_TypesKeyPresent()
    {
        var p = new SaikuroProvider("x");
        Assert.True(p.SchemaDict().ContainsKey("types"));
    }

    [Fact]
    public void SchemaDict_IdempotentWhenTrue()
    {
        var p = new SaikuroProvider("svc");
        p.Register(
            "fn",
            (IReadOnlyList<object?> _) => (object?)null,
            new RegisterOptions { Idempotent = true }
        );

        var ns = (Dictionary<string, object?>)p.SchemaDict()["namespaces"]!;
        var fns =
            (Dictionary<string, object?>)((Dictionary<string, object?>)ns["svc"]!)["functions"]!;
        var fn = (Dictionary<string, object?>)fns["fn"]!;
        Assert.True((bool)fn["idempotent"]!);
    }
}

//  TypeDescriptor tests

public class TypeDescriptorTests
{
    [Theory]
    [InlineData("bool")]
    [InlineData("i32")]
    [InlineData("i64")]
    [InlineData("f32")]
    [InlineData("f64")]
    [InlineData("string")]
    [InlineData("bytes")]
    [InlineData("any")]
    [InlineData("unit")]
    public void Primitive_ToWire_HasCorrectKindAndType(string typeName)
    {
        var td = new TypeDescriptor.Primitive(typeName);
        var wire = td.ToWire();
        Assert.Equal("primitive", wire["kind"]);
        Assert.Equal(typeName, wire["type"]);
    }

    [Fact]
    public void List_ToWire_HasKindAndItem()
    {
        var td = new TypeDescriptor.List(new TypeDescriptor.Primitive("i64"));
        var wire = td.ToWire();
        Assert.Equal("list", wire["kind"]);
        var item = (Dictionary<string, object?>)wire["item"]!;
        Assert.Equal("i64", item["type"]);
    }

    [Fact]
    public void Map_ToWire_HasKindKeyValue()
    {
        var td = new TypeDescriptor.Map(
            new TypeDescriptor.Primitive("string"),
            new TypeDescriptor.Primitive("i64")
        );
        var wire = td.ToWire();
        Assert.Equal("map", wire["kind"]);
        Assert.NotNull(wire["key"]);
        Assert.NotNull(wire["value"]);
    }

    [Fact]
    public void Optional_ToWire_HasKindAndInner()
    {
        var td = new TypeDescriptor.Optional(new TypeDescriptor.Primitive("string"));
        var wire = td.ToWire();
        Assert.Equal("optional", wire["kind"]);
        Assert.NotNull(wire["inner"]);
    }

    [Fact]
    public void Named_ToWire_HasKindAndName()
    {
        var td = new TypeDescriptor.Named("MyType");
        var wire = td.ToWire();
        Assert.Equal("named", wire["kind"]);
        Assert.Equal("MyType", wire["name"]);
    }

    [Fact]
    public void Stream_ToWire_HasKindAndItem()
    {
        var td = new TypeDescriptor.Stream(new TypeDescriptor.Primitive("i64"));
        var wire = td.ToWire();
        Assert.Equal("stream", wire["kind"]);
    }

    [Fact]
    public void Channel_ToWire_HasKindSendRecv()
    {
        var td = new TypeDescriptor.Channel(
            new TypeDescriptor.Primitive("string"),
            new TypeDescriptor.Primitive("i64")
        );
        var wire = td.ToWire();
        Assert.Equal("channel", wire["kind"]);
        Assert.NotNull(wire["send"]);
        Assert.NotNull(wire["recv"]);
    }

    [Fact]
    public void THelpers_AllBuildCorrectTypes()
    {
        Assert.IsType<TypeDescriptor.Primitive>(T.Bool());
        Assert.IsType<TypeDescriptor.Primitive>(T.I32());
        Assert.IsType<TypeDescriptor.Primitive>(T.I64());
        Assert.IsType<TypeDescriptor.Primitive>(T.F32());
        Assert.IsType<TypeDescriptor.Primitive>(T.F64());
        Assert.IsType<TypeDescriptor.Primitive>(T.String());
        Assert.IsType<TypeDescriptor.Primitive>(T.Bytes());
        Assert.IsType<TypeDescriptor.Primitive>(T.Any());
        Assert.IsType<TypeDescriptor.Primitive>(T.Unit());
        Assert.IsType<TypeDescriptor.List>(T.List(T.I64()));
        Assert.IsType<TypeDescriptor.Map>(T.Map(T.String(), T.I64()));
        Assert.IsType<TypeDescriptor.Optional>(T.Optional(T.String()));
        Assert.IsType<TypeDescriptor.Named>(T.Named("X"));
        Assert.IsType<TypeDescriptor.Stream>(T.Stream(T.I64()));
        Assert.IsType<TypeDescriptor.Channel>(T.Channel(T.String(), T.I64()));
    }
}

//  ServeOnAsync tests

public class ProviderServeOnTests
{
    [Fact]
    public async Task ServeOnAsync_SendsAnnounceEnvelope()
    {
        var (providerTransport, clientTransport) = InMemoryTransport.Pair();
        var p = new SaikuroProvider("svc");
        p.Register("fn", (IReadOnlyList<object?> _) => (object?)"ok");

        using var cts = new CancellationTokenSource();
        var serveTask = Task.Run(() => p.ServeOnAsync(providerTransport, cts.Token));

        // First message from provider should be the announce envelope.
        var announce = await clientTransport.RecvAsync();
        Assert.NotNull(announce);
        Assert.Equal("announce", announce!["type"]);
        Assert.Equal("$saikuro.announce", announce["target"]);

        // Send ack so the provider doesn't stall waiting for it.
        await clientTransport.SendAsync(new Dictionary<string, object?> { ["ok"] = true });

        // Cancel and clean up.
        cts.Cancel();
        await clientTransport.CloseAsync();
        try
        {
            await serveTask;
        }
        catch
        { /* ignore cancellation */
        }
    }

    [Fact]
    public async Task ServeOnAsync_DispatchesInvocation()
    {
        var (providerTransport, clientTransport) = InMemoryTransport.Pair();
        var p = new SaikuroProvider("svc");
        p.Register("ping", (IReadOnlyList<object?> _) => (object?)"pong");

        using var cts = new CancellationTokenSource();
        var serveTask = Task.Run(() => p.ServeOnAsync(providerTransport, cts.Token));

        // Consume the announce.
        var announce = await clientTransport.RecvAsync();
        Assert.NotNull(announce);
        await clientTransport.SendAsync(new Dictionary<string, object?> { ["ok"] = true });

        // Send a call invocation.
        var callEnv = Envelope.MakeCall("svc.ping", []);
        await clientTransport.SendAsync(callEnv.ToMsgpackDict());

        // Receive the response.
        var resp = await clientTransport.RecvAsync();
        Assert.NotNull(resp);
        Assert.True((bool)resp!["ok"]!);
        Assert.Equal("pong", resp["result"]);

        cts.Cancel();
        await clientTransport.CloseAsync();
        try
        {
            await serveTask;
        }
        catch
        { /* ignore */
        }
    }
}
