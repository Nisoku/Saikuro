// Saikuro async client.
//
// Multiplexes call/cast/stream/channel/batch/resource/log over one ITransport
// connection using invocation IDs as correlation keys.
//
// Usage:
//   var client = await SaikuroClient.ConnectAsync("tcp://localhost:7700");
//   var result = await client.CallAsync("math.add", new object?[] { 1, 2 });
//   await client.CloseAsync();

using System.Collections.Concurrent;

namespace Saikuro;

//  Internal routing interface

/// <summary>Used internally so both typed stream/channel impls can be stored uniformly.</summary>
internal interface IDeliverable
{
    void Deliver(ResponseEnvelope resp);
    void Close();
}

//  Base class for stream / channel handles

/// <summary>Shared async-enumerable plumbing for stream and channel types.</summary>
public abstract class BaseSaikuroHandle<T> : IAsyncEnumerable<T>, IDeliverable
{
    protected readonly string _id;
    private readonly System.Threading.Channels.Channel<ResponseEnvelope?> _ch =
        System.Threading.Channels.Channel.CreateUnbounded<ResponseEnvelope?>();
    protected bool _done;

    protected BaseSaikuroHandle(string id) => _id = id;

    /// <summary>The invocation ID that identifies this handle.</summary>
    public string InvocationId => _id;

    void IDeliverable.Deliver(ResponseEnvelope resp) => _ch.Writer.TryWrite(resp);
    void IDeliverable.Close() => _ch.Writer.TryComplete(
        new TransportException("ConnectionLost", "transport closed", new Dictionary<string, object?>())
    );

    public async IAsyncEnumerator<T> GetAsyncEnumerator(
        CancellationToken cancellationToken = default
    )
    {
        if (_done)
            yield break;
        await foreach (var item in _ch.Reader.ReadAllAsync(cancellationToken).ConfigureAwait(false))
        {
            if (item is null)
            {
                _done = true;
                yield break;
            }
            if (item.IsStreamEnd)
            {
                _done = true;
                yield break;
            }
            if (!item.Ok)
            {
                _done = true;
                var payload =
                    item.Error
                    ?? new ErrorPayload { Code = "Internal", Message = "stream ended with error" };
                throw SaikuroException.FromPayload(payload);
            }
            yield return (T)item.Result!;
        }
    }
}

//  SaikuroStream<T>

/// <summary>
/// An async-enumerable stream of values received from the provider.
/// Obtained from <see cref="SaikuroClient.StreamAsync{T}"/>.
/// </summary>
public sealed class SaikuroStream<T> : BaseSaikuroHandle<T>
{
    internal SaikuroStream(string id) : base(id) { }
}

//  SaikuroChannel<TIn, TOut>

/// <summary>
/// A bidirectional channel.  Received items are async-enumerable;
/// use <see cref="SendAsync"/> to send items to the provider.
/// Obtained from <see cref="SaikuroClient.ChannelAsync{TIn,TOut}"/>.
/// </summary>
public sealed class SaikuroChannel<TIn, TOut> : BaseSaikuroHandle<TIn>
{
    private readonly Func<string, object?, Task> _sendFn;

    internal SaikuroChannel(string id, Func<string, object?, Task> sendFn) : base(id)
    {
        _sendFn = sendFn;
    }

    /// <summary>Send a message to the provider side of the channel.</summary>
    public Task SendAsync(TOut value, CancellationToken ct = default)
    {
        if (_done)
            throw new InvalidOperationException("Channel is already closed.");
        return _sendFn(_id, value);
    }
}

//  ClientOptions

/// <summary>Options for <see cref="SaikuroClient"/>.</summary>
public sealed class ClientOptions
{
    /// <summary>
    /// Default timeout for <c>call</c> invocations.
    /// <see cref="TimeSpan.Zero"/> means no timeout (default).
    /// </summary>
    public TimeSpan DefaultTimeout { get; init; } = TimeSpan.Zero;
}

//  SaikuroClient

/// <summary>
/// Async Saikuro client over a single transport connection.
/// </summary>
public sealed class SaikuroClient : IAsyncDisposable
{
    private readonly ITransport _transport;
    private readonly ClientOptions _options;

    // Pending call futures keyed by invocation ID.
    private readonly ConcurrentDictionary<
        string,
        TaskCompletionSource<ResponseEnvelope>
    > _pendingCalls = new();

    // Open streams and channels keyed by invocation ID (stored as IDeliverable).
    private readonly ConcurrentDictionary<string, IDeliverable> _openStreams = new();
    private readonly ConcurrentDictionary<string, IDeliverable> _openChannels = new();

    private CancellationTokenSource? _recvCts;
    private Task? _recvLoop;
    private bool _connected;

    private SaikuroClient(ITransport transport, ClientOptions? options = null)
    {
        _transport = transport;
        _options = options ?? new ClientOptions();
    }

    // Factories

    /// <summary>Connect to a Saikuro runtime at <paramref name="address"/> and return a ready client.</summary>
    public static async Task<SaikuroClient> ConnectAsync(
        string address,
        ClientOptions? options = null,
        CancellationToken ct = default
    )
    {
        var transport = TransportFactory.MakeTransport(address);
        return await OpenOnAsync(transport, options, ct).ConfigureAwait(false);
    }

    /// <summary>Connect an already-instantiated transport and return a ready client.</summary>
    public static async Task<SaikuroClient> OpenOnAsync(
        ITransport transport,
        ClientOptions? options = null,
        CancellationToken ct = default
    )
    {
        var client = new SaikuroClient(transport, options);
        await client.OpenAsync(ct).ConfigureAwait(false);
        return client;
    }

    /// <summary>Construct a client from a transport without connecting.</summary>
    public static SaikuroClient FromTransport(
        ITransport transport,
        ClientOptions? options = null
    ) => new(transport, options);

    // Lifecycle

    /// <summary>Connect the transport and start the receive loop.</summary>
    public async Task OpenAsync(CancellationToken ct = default)
    {
        await _transport.ConnectAsync(ct).ConfigureAwait(false);
        _connected = true;
        _recvCts = new CancellationTokenSource();
        _recvLoop = Task.Run(() => RunRecvLoopAsync(_recvCts.Token));
    }

    /// <summary><c>true</c> if the client is currently connected.</summary>
    public bool Connected => _connected;

    /// <summary>Gracefully close the client and its transport.</summary>
    public async Task CloseAsync(CancellationToken ct = default)
    {
        _connected = false;
        _recvCts?.Cancel();
        await _transport.CloseAsync(ct).ConfigureAwait(false);
        TeardownPending(
            new TransportException(
                "ConnectionLost",
                "client closed",
                new Dictionary<string, object?>()
            )
        );
        if (_recvLoop is not null)
            await _recvLoop.ConfigureAwait(ConfigureAwaitOptions.SuppressThrowing);
    }

    public async ValueTask DisposeAsync() => await CloseAsync().ConfigureAwait(false);

    // Invocation API

    /// <summary>Perform a request/response call and return the result value.</summary>
    public async Task<object?> CallAsync(
        string target,
        IReadOnlyList<object?> args,
        string? capability = null,
        TimeSpan? timeout = null,
        CancellationToken ct = default
    )
    {
        var envelope = Envelope.MakeCall(target, args, capability);
        var effectiveTimeout = timeout ?? _options.DefaultTimeout;
        var resp = await SendAndWaitAsync(envelope, effectiveTimeout, ct).ConfigureAwait(false);
        if (!resp.Ok)
        {
            var p = resp.Error ?? new ErrorPayload { Code = "Internal", Message = "call failed" };
            throw SaikuroException.FromPayload(p);
        }
        return resp.Result;
    }

    /// <summary>Fire-and-forget invocation.  No response is expected.</summary>
    public Task CastAsync(
        string target,
        IReadOnlyList<object?> args,
        string? capability = null,
        CancellationToken ct = default
    )
    {
        var envelope = Envelope.MakeCast(target, args, capability);
        return _transport.SendAsync(envelope.ToMsgpackDict(), ct);
    }

    /// <summary>Invoke a resource provider and return the resulting <see cref="ResourceHandle"/>.</summary>
    public async Task<ResourceHandle> ResourceAsync(
        string target,
        IReadOnlyList<object?> args,
        string? capability = null,
        TimeSpan? timeout = null,
        CancellationToken ct = default
    )
    {
        var envelope = Envelope.MakeResource(target, args, capability);
        var effectiveTimeout = timeout ?? _options.DefaultTimeout;
        var resp = await SendAndWaitAsync(envelope, effectiveTimeout, ct).ConfigureAwait(false);
        if (!resp.Ok)
        {
            var p =
                resp.Error
                ?? new ErrorPayload { Code = "Internal", Message = "resource call failed" };
            throw SaikuroException.FromPayload(p);
        }
        if (
            resp.Result is not Dictionary<string, object?> map
            || ResourceHandle.FromMap(map) is not { } handle
        )
        {
            throw SaikuroException.FromPayload(
                new ErrorPayload
                {
                    Code = "ProviderError",
                    Message =
                        $"resource invocation for \"{target}\" returned an invalid or missing ResourceHandle",
                }
            );
        }
        return handle;
    }

    /// <summary>Forward a structured log record to the runtime log sink (fire-and-forget).</summary>
    public Task LogAsync(
        string level,
        string name,
        string msg,
        IReadOnlyDictionary<string, object?>? fields = null,
        CancellationToken ct = default
    )
    {
        var ts = DateTimeOffset.UtcNow.ToString("O");
        var logRecord = new Dictionary<string, object?>
        {
            [WireKey.Ts] = ts,
            [WireKey.Level] = level,
            [WireKey.Name] = name,
            [WireKey.Msg] = msg,
        };
        if (fields is { Count: > 0 })
            logRecord[WireKey.Fields] = fields;
        var envelope = new Dictionary<string, object?>
        {
            [WireKey.Version] = (int)Protocol.Version,
            [WireKey.Type] = "log",
            [WireKey.Id] = $"log-{ts}",
            [WireKey.Target] = "$log",
            [WireKey.Args] = new object?[] { logRecord },
        };
        return _transport.SendAsync(envelope, ct);
    }

    /// <summary>Execute multiple calls in a single round-trip.</summary>
    public async Task<IReadOnlyList<object?>> BatchAsync(
        IReadOnlyList<(string Target, IReadOnlyList<object?> Args, string? Capability)> calls,
        TimeSpan? timeout = null,
        CancellationToken ct = default
    )
    {
        var items = calls.Select(c => Envelope.MakeCall(c.Target, c.Args, c.Capability)).ToList();
        var batchEnvelope = Envelope.MakeBatch(items);
        var effectiveTimeout = timeout ?? _options.DefaultTimeout;
        var resp = await SendAndWaitAsync(batchEnvelope, effectiveTimeout, ct)
            .ConfigureAwait(false);
        if (!resp.Ok)
        {
            var p =
                resp.Error ?? new ErrorPayload { Code = "Internal", Message = "batch call failed" };
            throw SaikuroException.FromPayload(p);
        }
        return resp.Result is System.Collections.IList list
            ? list.Cast<object?>().ToList().AsReadOnly()
            : new List<object?> { resp.Result }.AsReadOnly();
    }

    /// <summary>Open a server-to-client stream. Returns an async-enumerable of items.</summary>
    public async Task<SaikuroStream<T>> StreamAsync<T>(
        string target,
        IReadOnlyList<object?> args,
        string? capability = null,
        CancellationToken ct = default
    )
    {
        var envelope = Envelope.MakeStreamOpen(target, args) with { Capability = capability };
        var handle = new SaikuroStream<T>(envelope.Id);
        _openStreams[envelope.Id] = (IDeliverable)handle;
        await _transport.SendAsync(envelope.ToMsgpackDict(), ct).ConfigureAwait(false);
        return handle;
    }

    /// <summary>Open a bidirectional channel.</summary>
    public async Task<SaikuroChannel<TIn, TOut>> ChannelAsync<TIn, TOut>(
        string target,
        IReadOnlyList<object?> args,
        string? capability = null,
        CancellationToken ct = default
    )
    {
        var envelope = Envelope.MakeChannelOpen(target, args) with { Capability = capability };
        var handle = new SaikuroChannel<TIn, TOut>(envelope.Id, ChannelSendAsync);
        _openChannels[envelope.Id] = (IDeliverable)handle;
        await _transport.SendAsync(envelope.ToMsgpackDict(), ct).ConfigureAwait(false);
        return handle;
    }

    // Internal

    private async Task<ResponseEnvelope> SendAndWaitAsync(
        Envelope envelope,
        TimeSpan timeout,
        CancellationToken ct
    )
    {
        var tcs = new TaskCompletionSource<ResponseEnvelope>(
            TaskCreationOptions.RunContinuationsAsynchronously
        );
        _pendingCalls[envelope.Id] = tcs;

        CancellationTokenSource? timeoutCts = null;
        CancellationTokenRegistration reg = default;

        if (timeout > TimeSpan.Zero)
        {
            timeoutCts = new CancellationTokenSource(timeout);
            reg = timeoutCts.Token.Register(() =>
            {
                if (_pendingCalls.TryRemove(envelope.Id, out _))
                    tcs.TrySetException(
                        new TimeoutException(
                            $"Call to \"{envelope.Target}\" timed out after {timeout.TotalMilliseconds}ms."
                        )
                    );
            });
        }

        try
        {
            await _transport.SendAsync(envelope.ToMsgpackDict(), ct).ConfigureAwait(false);
        }
        catch (Exception ex)
        {
            _pendingCalls.TryRemove(envelope.Id, out _);
            timeoutCts?.Dispose();
            tcs.TrySetException(ex);
        }

        try
        {
            return await tcs.Task.ConfigureAwait(false);
        }
        finally
        {
            reg.Dispose();
            timeoutCts?.Dispose();
        }
    }

    private Task ChannelSendAsync(string channelId, object? value)
    {
        var d = new Dictionary<string, object?>
        {
            [WireKey.Version] = (int)Protocol.Version,
            [WireKey.Type] = "channel",
            [WireKey.Id] = channelId,
            [WireKey.Target] = "",
            [WireKey.Args] = new object?[] { value },
        };
        return _transport.SendAsync(d);
    }

    private async Task RunRecvLoopAsync(CancellationToken ct)
    {
        try
        {
            while (!ct.IsCancellationRequested)
            {
                Dictionary<string, object?>? raw;
                try
                {
                    raw = await _transport.RecvAsync(ct).ConfigureAwait(false);
                }
                catch (OperationCanceledException)
                {
                    break;
                }
                catch (Exception ex)
                {
                    HandleClose(ex);
                    break;
                }

                if (raw is null)
                {
                    HandleClose(null);
                    break;
                }

                try
                {
                    var resp = ResponseEnvelope.FromMsgpackDict(raw);
                    DispatchResponse(resp);
                }
                catch (Exception ex)
                {
                    SaikuroLogger
                        .GetLogger("saikuro.client")
                        .Error("malformed inbound response, skipping", ex.Message);
                }
            }
        }
        catch (Exception ex)
        {
            HandleClose(ex);
        }
    }

    private void DispatchResponse(ResponseEnvelope resp)
    {
        var id = resp.Id;

        if (_pendingCalls.TryRemove(id, out var tcs))
        {
            tcs.TrySetResult(resp);
            return;
        }

        if (_openStreams.TryGetValue(id, out var stream))
        {
            stream.Deliver(resp);
            if (resp.IsStreamEnd || !resp.Ok)
                _openStreams.TryRemove(id, out _);
            return;
        }

        if (_openChannels.TryGetValue(id, out var channel))
        {
            channel.Deliver(resp);
            if (resp.IsStreamEnd || !resp.Ok)
                _openChannels.TryRemove(id, out _);
            return;
        }

        // Unknown ID:  silently ignore (late response after timeout, etc.).
    }

    private void HandleClose(Exception? ex)
    {
        _connected = false;
        var msg = ex?.Message ?? "transport closed unexpectedly";
        TeardownPending(
            new TransportException("ConnectionLost", msg, new Dictionary<string, object?>())
        );
    }

    private void TeardownPending(TransportException transportEx)
    {
        foreach (var kv in _pendingCalls)
            kv.Value.TrySetException(transportEx);
        _pendingCalls.Clear();

        foreach (var kv in _openStreams)
            kv.Value.Close();
        _openStreams.Clear();

        foreach (var kv in _openChannels)
            kv.Value.Close();
        _openChannels.Clear();
    }
}
