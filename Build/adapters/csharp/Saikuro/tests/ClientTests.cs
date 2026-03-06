// Tests for SaikuroClient

using Saikuro;

namespace Saikuro.Tests;

//  Test harness

/// <summary>
/// Connects a SaikuroClient to a SaikuroProvider via InMemoryTransport pair,
/// bypassing the announce handshake.
/// </summary>
internal sealed class ClientProviderHarness : IAsyncDisposable
{
    public SaikuroClient Client { get; }
    public SaikuroProvider Provider { get; }
    private readonly InMemoryTransport _providerTransport;
    private readonly CancellationTokenSource _cts = new();
    private readonly Task _serveTask;

    private ClientProviderHarness(
        SaikuroClient client,
        SaikuroProvider provider,
        InMemoryTransport providerTransport
    )
    {
        Client = client;
        Provider = provider;
        _providerTransport = providerTransport;
        _serveTask = Task.Run(() => ServeLoopAsync(_cts.Token));
    }

    public static async Task<ClientProviderHarness> CreateAsync(string ns = "test")
    {
        var (clientTransport, providerTransport) = InMemoryTransport.Pair();
        var provider = new SaikuroProvider(ns);
        var client = SaikuroClient.FromTransport(clientTransport);
        await client.OpenAsync();
        return new ClientProviderHarness(client, provider, providerTransport);
    }

    private async Task ServeLoopAsync(CancellationToken ct)
    {
        while (!ct.IsCancellationRequested)
        {
            Dictionary<string, object?>? raw;
            try
            {
                raw = await _providerTransport.RecvAsync(ct);
            }
            catch (OperationCanceledException)
            {
                break;
            }
            if (raw is null)
                break;

            Envelope envelope;
            try
            {
                envelope = Envelope.FromMsgpackDict(raw);
            }
            catch
            {
                continue;
            }

            // Fire-and-forget each dispatch.
            _ = Provider.DispatchAsync(envelope, _providerTransport, ct);
        }
    }

    public async ValueTask DisposeAsync()
    {
        _cts.Cancel();
        await Client.CloseAsync();
        try
        {
            await _serveTask;
        }
        catch
        { /* ignore */
        }
        _cts.Dispose();
    }
}

//  CallAsync tests

public class ClientCallTests
{
    [Fact]
    public async Task CallAsync_SyncHandler_ReturnsResult()
    {
        await using var h = await ClientProviderHarness.CreateAsync();
        h.Provider.Register(
            "add",
            (IReadOnlyList<object?> args) =>
                (object?)(Convert.ToInt64(args[0]) + Convert.ToInt64(args[1]))
        );

        var result = await h.Client.CallAsync("test.add", new object?[] { 3L, 4L });
        Assert.Equal(7L, result);
    }

    [Fact]
    public async Task CallAsync_AsyncHandler_ReturnsResult()
    {
        await using var h = await ClientProviderHarness.CreateAsync();
        h.Provider.Register(
            "greet",
            (Func<IReadOnlyList<object?>, Task<object?>>)(
                args => Task.FromResult<object?>($"Hello, {args[0]}")
            )
        );

        var result = await h.Client.CallAsync("test.greet", new object?[] { "world" });
        Assert.Equal("Hello, world", result);
    }

    [Fact]
    public async Task CallAsync_HandlerThrows_RaisesSaikuroException()
    {
        await using var h = await ClientProviderHarness.CreateAsync();
        h.Provider.Register(
            "boom",
            (Func<IReadOnlyList<object?>, object?>)(
                _ => throw new InvalidOperationException("exploded")
            )
        );

        var ex = await Record.ExceptionAsync(() => h.Client.CallAsync("test.boom", []));
        Assert.IsAssignableFrom<SaikuroException>(ex);
    }

    [Fact]
    public async Task CallAsync_HandlerThrows_ExceptionMessagePropagated()
    {
        await using var h = await ClientProviderHarness.CreateAsync();
        h.Provider.Register(
            "boom",
            (Func<IReadOnlyList<object?>, object?>)(
                _ => throw new InvalidOperationException("exploded")
            )
        );

        var ex = await Assert.ThrowsAsync<ProviderException>(() =>
            h.Client.CallAsync("test.boom", [])
        );
        Assert.Contains("exploded", ex.Message);
    }

    [Fact]
    public async Task CallAsync_MissingFunction_ThrowsFunctionNotFound()
    {
        await using var h = await ClientProviderHarness.CreateAsync();
        var ex = await Assert.ThrowsAsync<FunctionNotFoundException>(() =>
            h.Client.CallAsync("test.missing_fn", [])
        );
        Assert.Contains("missing_fn", ex.Message);
    }

    [Fact]
    public async Task CallAsync_NullResult_ReturnsNull()
    {
        await using var h = await ClientProviderHarness.CreateAsync();
        h.Provider.Register("nil", (IReadOnlyList<object?> _) => (object?)null);
        var result = await h.Client.CallAsync("test.nil", []);
        Assert.Null(result);
    }

    [Fact]
    public async Task CallAsync_ArgsPassedCorrectly()
    {
        await using var h = await ClientProviderHarness.CreateAsync();
        h.Provider.Register("echo", (IReadOnlyList<object?> args) => args.ToArray());
        var result = await h.Client.CallAsync("test.echo", new object?[] { "hi", 42L });
        Assert.NotNull(result);
        var arr = (object?[])result!;
        Assert.Equal("hi", arr[0]);
        Assert.Equal(42L, arr[1]);
    }
}

//  CallAsync timeout

public class ClientCallTimeoutTests
{
    [Fact]
    public async Task CallAsync_TimeoutExpires_ThrowsTimeoutException()
    {
        // No provider:  the call will never receive a response.
        var (transport, _) = InMemoryTransport.Pair();
        var client = SaikuroClient.FromTransport(transport);
        await client.OpenAsync();

        await Assert.ThrowsAsync<TimeoutException>(() =>
            client.CallAsync("nowhere.fn", [], timeout: TimeSpan.FromMilliseconds(30))
        );

        await client.CloseAsync();
    }

    [Fact]
    public async Task CallAsync_DefaultTimeout_Zero_NoTimeout()
    {
        // With no timeout the call should just block, not immediately throw.
        // We verify this by completing successfully with a real provider.
        await using var h = await ClientProviderHarness.CreateAsync();
        h.Provider.Register("fn", (IReadOnlyList<object?> _) => (object?)"ok");
        var result = await h.Client.CallAsync("test.fn", []);
        Assert.Equal("ok", result);
    }
}

//  CastAsync tests

public class ClientCastTests
{
    [Fact]
    public async Task CastAsync_ReturnsWithoutError()
    {
        var (a, _) = InMemoryTransport.Pair();
        var client = SaikuroClient.FromTransport(a);
        await client.OpenAsync();
        // Should not throw even with no provider.
        await client.CastAsync("events.fire", new object?[] { "click" });
        await client.CloseAsync();
    }

    [Fact]
    public async Task CastAsync_SendsEnvelopeWithTypeCast()
    {
        var (a, b) = InMemoryTransport.Pair();
        var client = SaikuroClient.FromTransport(a);
        await client.OpenAsync();

        await client.CastAsync("test.track", new object?[] { "event" });
        var raw = await b.RecvAsync();

        Assert.NotNull(raw);
        Assert.Equal("cast", raw!["type"]);
        Assert.Equal("test.track", raw["target"]);
        await client.CloseAsync();
    }
}

//  ResourceAsync tests

public class ClientResourceTests
{
    [Fact]
    public async Task ResourceAsync_DecodesResourceHandle()
    {
        await using var h = await ClientProviderHarness.CreateAsync();
        h.Provider.Register(
            "open",
            (IReadOnlyList<object?> _) =>
                (object?)
                    new Dictionary<string, object?>
                    {
                        ["id"] = "res-42",
                        ["mime_type"] = "text/plain",
                        ["size"] = 100L,
                    }
        );

        var handle = await h.Client.ResourceAsync("test.open", []);
        Assert.Equal("res-42", handle.Id);
        Assert.Equal("text/plain", handle.MimeType);
        Assert.Equal(100L, handle.Size);
    }

    [Fact]
    public async Task ResourceAsync_InvalidResult_ThrowsSaikuroException()
    {
        await using var h = await ClientProviderHarness.CreateAsync();
        h.Provider.Register("open", (IReadOnlyList<object?> _) => (object?)"not-a-handle");

        var ex = await Record.ExceptionAsync(() => h.Client.ResourceAsync("test.open", []));
        Assert.IsAssignableFrom<SaikuroException>(ex);
    }

    [Fact]
    public async Task ResourceAsync_ProviderError_ThrowsSaikuroException()
    {
        await using var h = await ClientProviderHarness.CreateAsync();
        h.Provider.Register(
            "open",
            (Func<IReadOnlyList<object?>, object?>)(_ => throw new FileNotFoundException("missing"))
        );

        var ex = await Record.ExceptionAsync(() => h.Client.ResourceAsync("test.open", []));
        Assert.IsAssignableFrom<SaikuroException>(ex);
    }
}

//  StreamAsync tests

public class ClientStreamTests
{
    [Fact]
    public async Task StreamAsync_YieldsAllItems()
    {
        await using var h = await ClientProviderHarness.CreateAsync();
        h.Provider.Register(
            "count",
            (Func<IReadOnlyList<object?>, IAsyncEnumerable<object?>>)(_ => CountAsync())
        );

        var stream = await h.Client.StreamAsync<long>("test.count", []);
        var items = new List<long>();
        await foreach (var item in stream)
            items.Add(item);

        Assert.Equal(new long[] { 1, 2, 3 }, items);
    }

    [Fact]
    public async Task StreamAsync_EmptyGenerator_ProducesNoItems()
    {
        await using var h = await ClientProviderHarness.CreateAsync();
        h.Provider.Register("empty", (IReadOnlyList<object?> _) => EmptyAsync());

        var stream = await h.Client.StreamAsync<object>("test.empty", []);
        var items = new List<object>();
        await foreach (var item in stream)
            items.Add(item!);

        Assert.Empty(items);
    }

    [Fact]
    public async Task StreamAsync_MidStreamException_ThrowsSaikuroException()
    {
        await using var h = await ClientProviderHarness.CreateAsync();
        h.Provider.Register("fail_after_one", (IReadOnlyList<object?> _) => FailAfterOneAsync());

        var stream = await h.Client.StreamAsync<long>("test.fail_after_one", []);
        var items = new List<long>();
        var ex = await Record.ExceptionAsync(async () =>
        {
            await foreach (var item in stream)
                items.Add(item);
        });
        Assert.IsAssignableFrom<SaikuroException>(ex);
        Assert.Equal(new long[] { 10 }, items);
    }

    private static async IAsyncEnumerable<object?> CountAsync()
    {
        yield return 1L;
        yield return 2L;
        yield return 3L;
        await Task.CompletedTask;
    }

    private static async IAsyncEnumerable<object?> EmptyAsync()
    {
        await Task.CompletedTask;
        yield break;
    }

    private static async IAsyncEnumerable<object?> FailAfterOneAsync()
    {
        yield return 10L;
        await Task.CompletedTask;
        throw new InvalidOperationException("mid-stream failure");
    }
}

//  ChannelAsync tests

public class ClientChannelTests
{
    [Fact]
    public async Task ChannelAsync_ReceivesItemsFromProvider()
    {
        await using var h = await ClientProviderHarness.CreateAsync();
        h.Provider.Register("echo_ch", (IReadOnlyList<object?> _) => EchoChannelAsync());

        var ch = await h.Client.ChannelAsync<string, string>("test.echo_ch", []);
        var received = new List<string>();
        await foreach (var item in ch)
            received.Add(item);

        Assert.Equal(new[] { "alpha", "beta" }, received);
    }

    [Fact]
    public async Task ChannelAsync_HasNonEmptyInvocationId()
    {
        await using var h = await ClientProviderHarness.CreateAsync();
        h.Provider.Register("ch", (IReadOnlyList<object?> _) => SingleItemAsync());

        var ch = await h.Client.ChannelAsync<string, string>("test.ch", []);
        Assert.NotEmpty(ch.InvocationId);
        await foreach (var _ in ch) { }
    }

    private static async IAsyncEnumerable<object?> EchoChannelAsync()
    {
        yield return "alpha";
        yield return "beta";
        await Task.CompletedTask;
    }

    private static async IAsyncEnumerable<object?> SingleItemAsync()
    {
        yield return "x";
        await Task.CompletedTask;
    }
}

//  BatchAsync tests

public class ClientBatchTests
{
    /// <summary>Spawn a background server that handles batch envelopes.</summary>
    private static (
        SaikuroClient client,
        Task serverTask,
        InMemoryTransport server
    ) MakeBatchHarness(Dictionary<string, Func<object?[], object?>> handlers)
    {
        var (clientTransport, serverTransport) = InMemoryTransport.Pair();
        var client = SaikuroClient.FromTransport(clientTransport);

        var serverTask = Task.Run(async () =>
        {
            while (true)
            {
                Dictionary<string, object?>? raw;
                try
                {
                    raw = await serverTransport.RecvAsync();
                }
                catch
                {
                    break;
                }
                if (raw is null)
                    break;
                var envelope = Envelope.FromMsgpackDict(raw);
                if (envelope.Type == InvocationType.Batch)
                {
                    var results = new List<object?>();
                    foreach (var item in envelope.BatchItems ?? [])
                    {
                        var fnName = item.Target.Split('.').Last();
                        if (handlers.TryGetValue(fnName, out var fn))
                        {
                            try
                            {
                                results.Add(fn(item.Args.Cast<object?>().ToArray()));
                            }
                            catch
                            {
                                results.Add(null);
                            }
                        }
                        else
                            results.Add(null);
                    }
                    await serverTransport.SendAsync(
                        new Dictionary<string, object?>
                        {
                            ["id"] = envelope.Id,
                            ["ok"] = true,
                            ["result"] = results.ToArray(),
                        }
                    );
                }
            }
        });

        return (client, serverTask, serverTransport);
    }

    [Fact]
    public async Task BatchAsync_ReturnsOrderedResults()
    {
        var (client, serverTask, serverTransport) = MakeBatchHarness(
            new()
            {
                ["add"] = args => Convert.ToInt64(args[0]) + Convert.ToInt64(args[1]),
                ["mul"] = args => Convert.ToInt64(args[0]) * Convert.ToInt64(args[1]),
            }
        );
        await client.OpenAsync();

        var results = await client.BatchAsync(
            new (string, IReadOnlyList<object?>, string?)[]
            {
                ("math.add", new object?[] { 2L, 3L }, null),
                ("math.mul", new object?[] { 4L, 5L }, null),
            }
        );

        Assert.Equal(2, results.Count);
        Assert.Equal(5L, results[0]);
        Assert.Equal(20L, results[1]);

        await client.CloseAsync();
        await serverTransport.CloseAsync();
        await serverTask;
    }

    [Fact]
    public async Task BatchAsync_SingleItem()
    {
        var (client, serverTask, serverTransport) = MakeBatchHarness(
            new() { ["echo"] = args => args[0] }
        );
        await client.OpenAsync();

        var results = await client.BatchAsync(
            new (string, IReadOnlyList<object?>, string?)[]
            {
                ("svc.echo", new object?[] { "hello" }, null),
            }
        );

        Assert.Single(results);
        Assert.Equal("hello", results[0]);

        await client.CloseAsync();
        await serverTransport.CloseAsync();
        await serverTask;
    }

    [Fact]
    public async Task BatchAsync_SendsBatchTypeEnvelope()
    {
        var (clientTransport, serverTransport) = InMemoryTransport.Pair();
        var client = SaikuroClient.FromTransport(clientTransport);
        await client.OpenAsync();

        var batchTask = client.BatchAsync(
            new (string, IReadOnlyList<object?>, string?)[]
            {
                ("ns.fn1", new object?[] { 1L }, null),
                ("ns.fn2", new object?[] { 2L }, null),
            },
            timeout: TimeSpan.FromSeconds(2)
        );

        var raw = await serverTransport.RecvAsync();
        Assert.NotNull(raw);
        Assert.Equal("batch", raw!["type"]);
        var items = ((System.Collections.IList)raw["batch_items"]!);
        Assert.Equal(2, items.Count);

        // Send back a response to unblock the batch task.
        await serverTransport.SendAsync(
            new Dictionary<string, object?>
            {
                ["id"] = raw["id"],
                ["ok"] = true,
                ["result"] = new object?[] { null, null },
            }
        );
        await batchTask;
        await client.CloseAsync();
    }

    [Fact]
    public async Task BatchAsync_Timeout_Throws()
    {
        var (transport, _) = InMemoryTransport.Pair();
        var client = SaikuroClient.FromTransport(transport);
        await client.OpenAsync();

        await Assert.ThrowsAsync<TimeoutException>(() =>
            client.BatchAsync(
                new (string, IReadOnlyList<object?>, string?)[] { ("svc.fn", [], null) },
                timeout: TimeSpan.FromMilliseconds(30)
            )
        );

        await client.CloseAsync();
    }

    [Fact]
    public async Task BatchAsync_ErrorResponse_ThrowsSaikuroException()
    {
        var (clientTransport, serverTransport) = InMemoryTransport.Pair();
        var client = SaikuroClient.FromTransport(clientTransport);
        await client.OpenAsync();

        var serverTask = Task.Run(async () =>
        {
            var raw = await serverTransport.RecvAsync();
            if (raw is null)
                return;
            await serverTransport.SendAsync(
                new Dictionary<string, object?>
                {
                    ["id"] = raw["id"],
                    ["ok"] = false,
                    ["error"] = new Dictionary<string, object?>
                    {
                        ["code"] = "MalformedEnvelope",
                        ["message"] = "empty batch",
                    },
                }
            );
        });

        var ex = await Record.ExceptionAsync(() =>
            client.BatchAsync(
                new (string, IReadOnlyList<object?>, string?)[] { ("svc.fn", [], null) }
            )
        );
        Assert.IsAssignableFrom<SaikuroException>(ex);

        await client.CloseAsync();
        await serverTask;
    }
}

//  LogAsync tests

public class ClientLogTests
{
    [Fact]
    public async Task LogAsync_ReturnsWithoutError()
    {
        var (a, _) = InMemoryTransport.Pair();
        var client = SaikuroClient.FromTransport(a);
        await client.OpenAsync();
        // Should not throw.
        await client.LogAsync("info", "test", "hello");
        await client.CloseAsync();
    }

    [Fact]
    public async Task LogAsync_SendsLogEnvelope()
    {
        var (a, b) = InMemoryTransport.Pair();
        var client = SaikuroClient.FromTransport(a);
        await client.OpenAsync();

        await client.LogAsync(
            "warn",
            "my.logger",
            "something happened",
            new Dictionary<string, object?> { ["detail"] = 42L }
        );

        var raw = await b.RecvAsync();
        Assert.NotNull(raw);
        Assert.Equal("log", raw!["type"]);
        Assert.Equal("$log", raw["target"]);

        var args = (object?[])raw["args"]!;
        var record = (Dictionary<string, object?>)args[0]!;
        Assert.Equal("warn", record["level"]);
        Assert.Equal("something happened", record["msg"]);
        Assert.Equal("my.logger", record["name"]);

        await client.CloseAsync();
    }
}

//  CloseAsync teardown

public class ClientCloseTeardownTests
{
    [Fact]
    public async Task CloseAsync_RejectsPendingCallWithTransportException()
    {
        var (a, _) = InMemoryTransport.Pair();
        var client = SaikuroClient.FromTransport(a);
        await client.OpenAsync();

        var callTask = client.CallAsync("nowhere.fn", []);
        // Let the call register before closing.
        await Task.Delay(10);
        await client.CloseAsync();

        await Assert.ThrowsAsync<TransportException>(async () => await callTask);
    }

    [Fact]
    public async Task CloseAsync_ConnectedFalseAfterClose()
    {
        var (a, _) = InMemoryTransport.Pair();
        var client = SaikuroClient.FromTransport(a);
        await client.OpenAsync();
        Assert.True(client.Connected);
        await client.CloseAsync();
        Assert.False(client.Connected);
    }

    [Fact]
    public async Task DisposeAsync_ClosesGracefully()
    {
        var (a, _) = InMemoryTransport.Pair();
        await using var client = SaikuroClient.FromTransport(a);
        await client.OpenAsync();
        // Dispose should not throw.
    }
}

//  Concurrent calls

public class ClientConcurrentCallTests
{
    [Fact]
    public async Task ConcurrentCalls_ResolveIndependently()
    {
        await using var h = await ClientProviderHarness.CreateAsync();
        h.Provider.Register(
            "double",
            (IReadOnlyList<object?> args) => (object?)(Convert.ToInt64(args[0]) * 2)
        );

        var tasks = new[] { 1L, 2L, 3L, 4L }.Select(n =>
            h.Client.CallAsync("test.double", new object?[] { n })
        );

        var results = await Task.WhenAll(tasks);
        Assert.Equal(
            new object?[] { 2L, 4L, 6L, 8L },
            results.OrderBy(x => Convert.ToInt64(x)).ToArray()
        );
    }
}

//  Factory methods

public class ClientFactoryTests
{
    [Fact]
    public async Task FromTransport_CreatesClientWithoutConnecting()
    {
        var (a, _) = InMemoryTransport.Pair();
        var client = SaikuroClient.FromTransport(a);
        // Not connected yet:  Connected should be false.
        Assert.False(client.Connected);
        await client.OpenAsync();
        Assert.True(client.Connected);
        await client.CloseAsync();
    }

    [Fact]
    public async Task OpenOnAsync_ConnectsAndStartsRecvLoop()
    {
        var (a, _) = InMemoryTransport.Pair();
        var client = await SaikuroClient.OpenOnAsync(a);
        Assert.True(client.Connected);
        await client.CloseAsync();
    }
}
