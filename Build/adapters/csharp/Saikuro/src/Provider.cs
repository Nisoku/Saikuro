// Saikuro provider:  register C# functions and serve them to the runtime.
//
// Usage:
//
//   var provider = new SaikuroProvider("math");
//
//   provider.Register("add", (IReadOnlyList<object?> args) =>
//       (long)args[0]! + (long)args[1]!);
//
//   await provider.ServeAsync("tcp://localhost:7700");

namespace Saikuro;

//  Handler delegates

/// <summary>A synchronous or async function handler.</summary>
public delegate Task<object?> AsyncHandler(IReadOnlyList<object?> args, CancellationToken ct);

/// <summary>An async-enumerable (streaming) function handler.</summary>
public delegate IAsyncEnumerable<object?> StreamHandler(
    IReadOnlyList<object?> args,
    CancellationToken ct
);

//  Registration metadata

/// <summary>Options supplied at <see cref="SaikuroProvider.Register"/> time.</summary>
public sealed class RegisterOptions
{
    public string? Doc { get; init; }
    public IReadOnlyList<string>? Capabilities { get; init; }
    public IReadOnlyList<ArgDescriptor>? Args { get; init; }
    public TypeDescriptor? Returns { get; init; }
    public bool? Idempotent { get; init; }
    public string Visibility { get; init; } = "public";
}

/// <summary>Describes a single function argument in the schema.</summary>
public sealed class ArgDescriptor
{
    public string Name { get; init; } = "";
    public TypeDescriptor? Type { get; init; }
    public bool Optional { get; init; }
    public string? Doc { get; init; }
    public object? Default { get; init; }
}

//  TypeDescriptor and builder

/// <summary>Represents a Saikuro type in the schema.</summary>
public abstract class TypeDescriptor
{
    private TypeDescriptor() { }

    /// <summary>Primitive type (bool, i32, i64, f32, f64, string, bytes, any, unit).</summary>
    public sealed class Primitive : TypeDescriptor
    {
        public string Type { get; }

        public Primitive(string type) => Type = type;

        public override Dictionary<string, object?> ToWire() =>
            new() { [WireKey.Kind] = "primitive", [WireKey.Type] = Type };
    }

    /// <summary>Homogeneous list of <see cref="Item"/>.</summary>
    public sealed class List : TypeDescriptor
    {
        public TypeDescriptor Item { get; }

        public List(TypeDescriptor item) => Item = item;

        public override Dictionary<string, object?> ToWire() =>
            new() { [WireKey.Kind] = "list", [WireKey.Item] = Item.ToWire() };
    }

    /// <summary>Map with typed keys and values.</summary>
    public sealed class Map : TypeDescriptor
    {
        public TypeDescriptor Key { get; }
        public TypeDescriptor Value { get; }

        public Map(TypeDescriptor key, TypeDescriptor value)
        {
            Key = key;
            Value = value;
        }

        public override Dictionary<string, object?> ToWire() =>
            new()
            {
                [WireKey.Kind] = "map",
                [WireKey.Key] = Key.ToWire(),
                [WireKey.Value] = Value.ToWire(),
            };
    }

    /// <summary>Optional wrapper.</summary>
    public sealed class Optional : TypeDescriptor
    {
        public TypeDescriptor Inner { get; }

        public Optional(TypeDescriptor inner) => Inner = inner;

        public override Dictionary<string, object?> ToWire() =>
            new() { [WireKey.Kind] = "optional", [WireKey.Inner] = Inner.ToWire() };
    }

    /// <summary>Named (user-defined) type.</summary>
    public sealed class Named : TypeDescriptor
    {
        public string Name { get; }

        public Named(string name) => Name = name;

        public override Dictionary<string, object?> ToWire() =>
            new() { [WireKey.Kind] = "named", [WireKey.Name] = Name };
    }

    /// <summary>Server-to-client stream of items.</summary>
    public sealed class Stream : TypeDescriptor
    {
        public TypeDescriptor Item { get; }

        public Stream(TypeDescriptor item) => Item = item;

        public override Dictionary<string, object?> ToWire() =>
            new() { [WireKey.Kind] = "stream", [WireKey.Item] = Item.ToWire() };
    }

    /// <summary>Bidirectional channel.</summary>
    public sealed class Channel : TypeDescriptor
    {
        public TypeDescriptor Send { get; }
        public TypeDescriptor Recv { get; }

        public Channel(TypeDescriptor send, TypeDescriptor recv)
        {
            Send = send;
            Recv = recv;
        }

        public override Dictionary<string, object?> ToWire() =>
            new()
            {
                [WireKey.Kind] = "channel",
                [WireKey.Send] = Send.ToWire(),
                [WireKey.Recv] = Recv.ToWire(),
            };
    }

    /// <summary>Convert to the wire-format dictionary.</summary>
    public abstract Dictionary<string, object?> ToWire();
}

/// <summary>Builder helpers:  mirrors the TypeScript <c>t.*</c> namespace.</summary>
public static class T
{
    public static TypeDescriptor Bool() => new TypeDescriptor.Primitive("bool");

    public static TypeDescriptor I32() => new TypeDescriptor.Primitive("i32");

    public static TypeDescriptor I64() => new TypeDescriptor.Primitive("i64");

    public static TypeDescriptor F32() => new TypeDescriptor.Primitive("f32");

    public static TypeDescriptor F64() => new TypeDescriptor.Primitive("f64");

    public static TypeDescriptor String() => new TypeDescriptor.Primitive("string");

    public static TypeDescriptor Bytes() => new TypeDescriptor.Primitive("bytes");

    public static TypeDescriptor Any() => new TypeDescriptor.Primitive("any");

    public static TypeDescriptor Unit() => new TypeDescriptor.Primitive("unit");

    public static TypeDescriptor List(TypeDescriptor item) => new TypeDescriptor.List(item);

    public static TypeDescriptor Map(TypeDescriptor key, TypeDescriptor value) =>
        new TypeDescriptor.Map(key, value);

    public static TypeDescriptor Optional(TypeDescriptor inner) =>
        new TypeDescriptor.Optional(inner);

    public static TypeDescriptor Named(string name) => new TypeDescriptor.Named(name);

    public static TypeDescriptor Stream(TypeDescriptor item) => new TypeDescriptor.Stream(item);

    public static TypeDescriptor Channel(TypeDescriptor send, TypeDescriptor recv) =>
        new TypeDescriptor.Channel(send, recv);
}

//  Handler storage

internal sealed class HandlerEntry
{
    internal AsyncHandler? Async { get; init; }
    internal StreamHandler? Streaming { get; init; }
    internal RegisterOptions Options { get; init; } = new();
    internal bool IsStreaming => Streaming is not null;
}

//  SaikuroProvider

/// <summary>
/// Exposes C# functions as invokable Saikuro functions within a single namespace.
/// </summary>
public sealed class SaikuroProvider
{
    private readonly string _namespace;
    private readonly Dictionary<string, HandlerEntry> _handlers = new();
    private static readonly SaikuroLogger Log = SaikuroLogger.GetLogger("saikuro.provider");

    public SaikuroProvider(string @namespace) => _namespace = @namespace;

    /// <summary>The namespace this provider publishes under.</summary>
    public string Namespace => _namespace;

    //  Registration

    /// <summary>
    /// Register a synchronous or async function.
    /// The <paramref name="handler"/> receives the args list and must return
    /// the result as <c>object?</c>.
    /// </summary>
    public SaikuroProvider Register(
        string name,
        Func<IReadOnlyList<object?>, object?> handler,
        RegisterOptions? options = null
    )
    {
        _handlers[name] = new HandlerEntry
        {
            Async = (args, _) => Task.FromResult(handler(args)),
            Options = options ?? new RegisterOptions(),
        };
        return this;
    }

    /// <summary>Register an async function.</summary>
    public SaikuroProvider Register(
        string name,
        Func<IReadOnlyList<object?>, Task<object?>> handler,
        RegisterOptions? options = null
    )
    {
        _handlers[name] = new HandlerEntry
        {
            Async = (args, ct) => handler(args),
            Options = options ?? new RegisterOptions(),
        };
        return this;
    }

    /// <summary>Register an async-enumerable (streaming) function.</summary>
    public SaikuroProvider Register(
        string name,
        Func<IReadOnlyList<object?>, IAsyncEnumerable<object?>> handler,
        RegisterOptions? options = null
    )
    {
        _handlers[name] = new HandlerEntry
        {
            Streaming = (args, _) => handler(args),
            Options = options ?? new RegisterOptions(),
        };
        return this;
    }

    //  Schema

    /// <summary>
    /// Build the schema announcement dictionary for this provider.
    /// </summary>
    public Dictionary<string, object?> SchemaDict()
    {
        var functions = new Dictionary<string, object?>();
        foreach (var (name, entry) in _handlers)
        {
            var opts = entry.Options;
            var argList = new List<object?>();
            if (opts.Args is not null)
            {
                foreach (var a in opts.Args)
                {
                    var wireArg = new Dictionary<string, object?>
                    {
                        [WireKey.Name] = a.Name,
                        [WireKey.Type] = (a.Type ?? T.Any()).ToWire(),
                    };
                    if (a.Optional)
                        wireArg[WireKey.Optional] = true;
                    if (a.Doc is not null)
                        wireArg[WireKey.Doc] = a.Doc;
                    if (a.Default is not null)
                        wireArg[WireKey.Default] = a.Default;
                    argList.Add(wireArg);
                }
            }

            var fn = new Dictionary<string, object?>
            {
                [WireKey.Args] = argList,
                [WireKey.Returns] = (opts.Returns ?? T.Any()).ToWire(),
                [WireKey.Visibility] = opts.Visibility,
                [WireKey.Capabilities] =
                    (IReadOnlyList<object?>)(opts.Capabilities?.Cast<object?>().ToList() ?? []),
            };
            if (opts.Doc is not null)
                fn[WireKey.Doc] = opts.Doc;
            if (opts.Idempotent.HasValue)
                fn[WireKey.Idempotent] = opts.Idempotent.Value;
            functions[name] = fn;
        }

        return new Dictionary<string, object?>
        {
            [WireKey.Version] = 1,
            [WireKey.Namespaces] = new Dictionary<string, object?>
            {
                [_namespace] = new Dictionary<string, object?> { [WireKey.Functions] = functions },
            },
            [WireKey.Types] = new Dictionary<string, object?>(),
        };
    }

    //  Dispatch

    /// <summary>
    /// Dispatch a single inbound invocation envelope.
    /// Called by the serve loop; also usable directly in tests.
    /// </summary>
    public async Task DispatchAsync(
        Envelope envelope,
        ITransport transport,
        CancellationToken ct = default
    )
    {
        // Extract the local function name (last segment of "namespace.fn_name").
        var target = envelope.Target;
        var dot = target.LastIndexOf('.');
        var fnName = dot >= 0 ? target[(dot + 1)..] : target;

        if (!_handlers.TryGetValue(fnName, out var entry))
        {
            await SendErrorAsync(
                    transport,
                    envelope.Id,
                    "FunctionNotFound",
                    $"no handler registered for '{target}'",
                    ct: ct
                )
                .ConfigureAwait(false);
            return;
        }

        try
        {
            if (entry.IsStreaming)
            {
                await DispatchStreamAsync(
                        envelope,
                        entry.Streaming!(envelope.Args, ct),
                        transport,
                        ct
                    )
                    .ConfigureAwait(false);
                return;
            }

            var result = await entry.Async!(envelope.Args, ct).ConfigureAwait(false);
            await SendOkAsync(transport, envelope.Id, result, ct).ConfigureAwait(false);
        }
        catch (SaikuroException saikEx)
        {
            await SendErrorAsync(
                    transport,
                    envelope.Id,
                    saikEx.Code,
                    saikEx.Message,
                    saikEx.Details,
                    ct
                )
                .ConfigureAwait(false);
        }
        catch (Exception ex)
        {
            await SendErrorAsync(transport, envelope.Id, "ProviderError", ex.Message, ct: ct)
                .ConfigureAwait(false);
        }
    }

    private static async Task DispatchStreamAsync(
        Envelope envelope,
        IAsyncEnumerable<object?> gen,
        ITransport transport,
        CancellationToken ct
    )
    {
        ulong seq = 0;
        try
        {
            await foreach (var item in gen.WithCancellation(ct).ConfigureAwait(false))
            {
                await transport
                    .SendAsync(
                        new Dictionary<string, object?>
                        {
                            [WireKey.Id] = envelope.Id,
                            [WireKey.Ok] = true,
                            [WireKey.Result] = item,
                            [WireKey.Seq] = (long)seq,
                        },
                        ct
                    )
                    .ConfigureAwait(false);
                seq++;
            }
            // End-of-stream sentinel.
            await transport
                .SendAsync(
                    new Dictionary<string, object?>
                    {
                        [WireKey.Id] = envelope.Id,
                        [WireKey.Ok] = true,
                        [WireKey.Seq] = (long)seq,
                        [WireKey.StreamControl] = "end",
                    },
                    ct
                )
                .ConfigureAwait(false);
        }
        catch (Exception ex)
        {
            await SendErrorAsync(transport, envelope.Id, "ProviderError", ex.Message, ct: ct)
                .ConfigureAwait(false);
            await transport
                .SendAsync(
                    new Dictionary<string, object?>
                    {
                        [WireKey.Id] = envelope.Id,
                        [WireKey.Ok] = false,
                        [WireKey.Seq] = (long)seq,
                        [WireKey.StreamControl] = "abort",
                    },
                    ct
                )
                .ConfigureAwait(false);
        }
    }

    //  Server loop

    /// <summary>Connect to the runtime at <paramref name="address"/> and serve invocations.</summary>
    public async Task ServeAsync(string address, CancellationToken ct = default)
    {
        var transport = TransportFactory.MakeTransport(address);
        await transport.ConnectAsync(ct).ConfigureAwait(false);
        try
        {
            await AnnounceAsync(transport, ct).ConfigureAwait(false);
            await RunServeLoopAsync(transport, ct).ConfigureAwait(false);
        }
        finally
        {
            await transport.CloseAsync(CancellationToken.None).ConfigureAwait(false);
        }
    }

    /// <summary>Serve invocations on an already-connected transport.</summary>
    public async Task ServeOnAsync(ITransport transport, CancellationToken ct = default)
    {
        await AnnounceAsync(transport, ct).ConfigureAwait(false);
        await RunServeLoopAsync(transport, ct).ConfigureAwait(false);
    }

    private async Task RunServeLoopAsync(ITransport transport, CancellationToken ct)
    {
        while (!ct.IsCancellationRequested)
        {
            Dictionary<string, object?>? raw;
            try
            {
                raw = await transport.RecvAsync(ct).ConfigureAwait(false);
            }
            catch (OperationCanceledException)
            {
                break;
            }
            catch (Exception ex)
            {
                Log.Error("transport recv error", ex.Message);
                break;
            }

            if (raw is null)
                break; // EOF

            Envelope envelope;
            try
            {
                envelope = Envelope.FromMsgpackDict(raw);
            }
            catch (Exception ex)
            {
                Log.Error("malformed inbound envelope, skipping", ex.Message);
                continue;
            }

            // Fire dispatch without awaiting :  allows concurrent invocations.
            _ = DispatchAsync(envelope, transport, ct)
                .ContinueWith(
                    t =>
                    {
                        if (t.IsFaulted)
                            Log.Error(
                                $"unhandled dispatch exception for target '{envelope.Target}'",
                                t.Exception?.InnerException?.Message ?? "unknown"
                            );
                    },
                    TaskScheduler.Default
                );
        }
    }

    private async Task AnnounceAsync(ITransport transport, CancellationToken ct)
    {
        try
        {
            var schema = SchemaDict();
            var envelope = Envelope.MakeAnnounce(schema);
            await transport.SendAsync(envelope.ToMsgpackDict(), ct).ConfigureAwait(false);

            using var timeoutCts = new CancellationTokenSource(TimeSpan.FromSeconds(5));
            using var linked = CancellationTokenSource.CreateLinkedTokenSource(
                ct,
                timeoutCts.Token
            );
            try
            {
                var ack = await transport.RecvAsync(linked.Token).ConfigureAwait(false);
                if (ack is not null && ack.TryGetValue(WireKey.Ok, out var okVal) && okVal is true)
                    Log.Debug("schema announce acknowledged");
                else
                    Log.Warn("schema announce rejected by runtime");
            }
            catch (OperationCanceledException)
            {
                Log.Warn("schema announce: timed out waiting for ack");
            }
        }
        catch (Exception ex)
        {
            Log.Warn($"schema announce failed (continuing anyway): {ex.Message}");
        }
    }

    //  Wire helpers

    private static Task SendOkAsync(
        ITransport transport,
        string id,
        object? result,
        CancellationToken ct
    ) =>
        transport.SendAsync(
            new Dictionary<string, object?>
            {
                [WireKey.Id] = id,
                [WireKey.Ok] = true,
                [WireKey.Result] = result,
            },
            ct
        );

    private static Task SendErrorAsync(
        ITransport transport,
        string id,
        string code,
        string message,
        IReadOnlyDictionary<string, object?>? details = null,
        CancellationToken ct = default
    )
    {
        var error = new Dictionary<string, object?> { [WireKey.Code] = code, [WireKey.Message] = message };
        if (details is { Count: > 0 })
            error[WireKey.Details] = details;
        return transport.SendAsync(
            new Dictionary<string, object?>
            {
                [WireKey.Id] = id,
                [WireKey.Ok] = false,
                [WireKey.Error] = error,
            },
            ct
        );
    }
}
