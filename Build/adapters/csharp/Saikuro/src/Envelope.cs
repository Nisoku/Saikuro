// Saikuro wire protocol envelope types.
//
// Mirrors the Rust saikuro_core::envelope module exactly.
// Wire format: MessagePack named-map (string keys).
// Optional fields are omitted from the wire when absent (never serialised as null keys).

using System.Collections.Generic;
using MessagePack;

namespace Saikuro;

// Protocol constant

public static class Protocol
{
    public const uint Version = 1u;
}

// InvocationType

/// <summary>Discriminator that tells the runtime how to handle a message.</summary>
public enum InvocationType
{
    Call,
    Cast,
    Stream,
    Channel,
    Batch,
    Resource,
    Log,
    Announce,
}

internal static class InvocationTypeExt
{
    internal static string ToWire(this InvocationType t) =>
        t switch
        {
            InvocationType.Call => "call",
            InvocationType.Cast => "cast",
            InvocationType.Stream => "stream",
            InvocationType.Channel => "channel",
            InvocationType.Batch => "batch",
            InvocationType.Resource => "resource",
            InvocationType.Log => "log",
            InvocationType.Announce => "announce",
            _ => throw new InvalidOperationException($"Unknown InvocationType: {t}"),
        };

    internal static InvocationType FromWire(string s) =>
        s switch
        {
            "call" => InvocationType.Call,
            "cast" => InvocationType.Cast,
            "stream" => InvocationType.Stream,
            "channel" => InvocationType.Channel,
            "batch" => InvocationType.Batch,
            "resource" => InvocationType.Resource,
            "log" => InvocationType.Log,
            "announce" => InvocationType.Announce,
            _ => throw new InvalidOperationException($"Unknown InvocationType wire value: {s}"),
        };
}

// StreamControl

/// <summary>Lifecycle / backpressure control signals on a stream or channel.</summary>
public enum StreamControl
{
    End,
    Pause,
    Resume,
    Abort,
}

internal static class StreamControlExt
{
    internal static string ToWire(this StreamControl sc) =>
        sc switch
        {
            StreamControl.End => "end",
            StreamControl.Pause => "pause",
            StreamControl.Resume => "resume",
            StreamControl.Abort => "abort",
            _ => throw new InvalidOperationException($"Unknown StreamControl: {sc}"),
        };

    internal static StreamControl FromWire(string s) =>
        s switch
        {
            "end" => StreamControl.End,
            "pause" => StreamControl.Pause,
            "resume" => StreamControl.Resume,
            "abort" => StreamControl.Abort,
            _ => throw new InvalidOperationException($"Unknown StreamControl wire value: {s}"),
        };
}

// ResourceHandle

/// <summary>Opaque reference to large or external data.</summary>
public sealed class ResourceHandle
{
    public string Id { get; init; } = "";
    public string? MimeType { get; init; }
    public long? Size { get; init; }
    public string? Uri { get; init; }

    /// <summary>Decode from a raw MessagePack map (as decoded by the transport layer).</summary>
    public static ResourceHandle? FromMap(Dictionary<string, object?> map)
    {
        if (!map.TryGetValue(WireKey.Id, out var rawId) || rawId is not string id)
            return null;

        string? mime = map.TryGetValue(WireKey.MimeType, out var m) && m is string ms ? ms : null;
        long? size = map.TryGetValue(WireKey.Size, out var s)
            ? s switch
            {
                long l => l,
                int i => (long)i,
                ulong u when u <= long.MaxValue => (long)u,
                _ => (long?)null,
            }
            : null;
        string? uri = map.TryGetValue(WireKey.Uri, out var u2) && u2 is string us ? us : null;

        return new ResourceHandle
        {
            Id = id,
            MimeType = mime,
            Size = size,
            Uri = uri,
        };
    }
}

// Envelope (outbound)

/// <summary>Outbound invocation envelope. Serialised to/from a MessagePack map.</summary>
public sealed record Envelope
{
    public uint Version { get; init; } = Protocol.Version;
    public InvocationType Type { get; init; }
    public string Id { get; init; } = "";
    public string Target { get; init; } = "";
    public IReadOnlyList<object?> Args { get; init; } = [];
    public IReadOnlyDictionary<string, object?>? Meta { get; init; }
    public string? Capability { get; init; }
    public IReadOnlyList<Envelope>? BatchItems { get; init; }
    public StreamControl? StreamControlValue { get; init; }
    public ulong? Seq { get; init; }

    // Factories

    public static Envelope MakeCall(
        string target,
        IReadOnlyList<object?> args,
        string? capability = null
    ) =>
        new()
        {
            Version = Protocol.Version,
            Type = InvocationType.Call,
            Id = NewId(),
            Target = target,
            Args = args,
            Capability = capability,
        };

    public static Envelope MakeCast(
        string target,
        IReadOnlyList<object?> args,
        string? capability = null
    ) => MakeCall(target, args, capability) with { Type = InvocationType.Cast };

    public static Envelope MakeStreamOpen(string target, IReadOnlyList<object?> args) =>
        MakeCall(target, args) with
        {
            Type = InvocationType.Stream,
        };

    public static Envelope MakeChannelOpen(string target, IReadOnlyList<object?> args) =>
        MakeCall(target, args) with
        {
            Type = InvocationType.Channel,
        };

    public static Envelope MakeResource(
        string target,
        IReadOnlyList<object?> args,
        string? capability = null
    ) => MakeCall(target, args, capability) with { Type = InvocationType.Resource };

    public static Envelope MakeBatch(IReadOnlyList<Envelope> items) =>
        new()
        {
            Version = Protocol.Version,
            Type = InvocationType.Batch,
            Id = NewId(),
            Target = "",
            Args = [],
            BatchItems = items,
        };

    public static Envelope MakeAnnounce(object schemaDict) =>
        new()
        {
            Version = Protocol.Version,
            Type = InvocationType.Announce,
            Id = NewId(),
            Target = "$saikuro.announce",
            Args = [schemaDict],
        };

    // Serialisation

    /// <summary>Serialise to the canonical MessagePack-ready dictionary.</summary>
    public Dictionary<string, object?> ToMsgpackDict()
    {
        var d = new Dictionary<string, object?>
        {
            [WireKey.Version] = (int)Version,
            [WireKey.Type] = Type.ToWire(),
            [WireKey.Id] = Id,
            [WireKey.Target] = Target,
            [WireKey.Args] = Args,
        };
        if (Meta is { Count: > 0 })
            d[WireKey.Meta] = Meta;
        if (Capability is not null)
            d[WireKey.Capability] = Capability;
        if (BatchItems is not null)
            d[WireKey.BatchItems] = BatchItems.Select(e => e.ToMsgpackDict()).ToList();
        if (StreamControlValue.HasValue)
            d[WireKey.StreamControl] = StreamControlValue.Value.ToWire();
        if (Seq.HasValue)
            d[WireKey.Seq] = (long)Seq.Value;
        return d;
    }

    /// <summary>Deserialise from a raw MessagePack map.</summary>
    public static Envelope FromMsgpackDict(Dictionary<string, object?> d)
    {
        var typeStr = (string)d[WireKey.Type]!;
        var sc =
            d.TryGetValue(WireKey.StreamControl, out var scRaw) && scRaw is string scs
                ? StreamControlExt.FromWire(scs)
                : (StreamControl?)null;
        var seq = d.TryGetValue(WireKey.Seq, out var seqRaw)
            ? seqRaw switch
            {
                long l when l >= 0 => (ulong)l,
                int i when i >= 0 => (ulong)i,
                ulong u => u,
                _ => (ulong?)null,
            }
            : null;
        var args =
            d.TryGetValue(WireKey.Args, out var argsRaw) && argsRaw is IList<object?> al
                ? (IReadOnlyList<object?>)al.ToList()
                : (IReadOnlyList<object?>)[];
        var meta =
            d.TryGetValue(WireKey.Meta, out var metaRaw) && metaRaw is Dictionary<string, object?> md
                ? md
                : null;
        List<Envelope>? items = null;
        if (d.TryGetValue(WireKey.BatchItems, out var biRaw) && biRaw is System.Collections.IList biList)
            items = biList
                .Cast<object?>()
                .OfType<Dictionary<string, object?>>()
                .Select(FromMsgpackDict)
                .ToList();

        return new Envelope
        {
            Version = d.TryGetValue(WireKey.Version, out var v)
                ? (uint)Convert.ToInt32(v)
                : Protocol.Version,
            Type = InvocationTypeExt.FromWire(typeStr),
            Id = (string)d[WireKey.Id]!,
            Target = (string)d[WireKey.Target]!,
            Args = args,
            Meta = meta,
            Capability = d.TryGetValue(WireKey.Capability, out var cap) ? cap as string : null,
            BatchItems = items,
            StreamControlValue = sc,
            Seq = seq,
        };
    }

    // Helpers

    private int? LastDotIndex => Target.LastIndexOf('.') is int i and >= 0 ? i : null;

    public string? Namespace => LastDotIndex is { } i ? Target[..i] : null;

    public string? FunctionName => LastDotIndex is { } i ? Target[(i + 1)..] : null;

    private static string NewId() => Guid.NewGuid().ToString();
}

// ResponseEnvelope (inbound) 

/// <summary>Inbound response envelope.</summary>
public sealed class ResponseEnvelope
{
    public string Id { get; init; } = "";
    public bool Ok { get; init; }
    public object? Result { get; init; }
    public ErrorPayload? Error { get; init; }
    public ulong? Seq { get; init; }
    public StreamControl? StreamControlValue { get; init; }

    public bool IsStreamEnd => StreamControlValue is StreamControl.End or StreamControl.Abort;

    /// <summary>Deserialise from a raw MessagePack map.</summary>
    public static ResponseEnvelope FromMsgpackDict(Dictionary<string, object?> d)
    {
        ErrorPayload? err = null;
        if (d.TryGetValue(WireKey.Error, out var errRaw) && errRaw is Dictionary<string, object?> em)
            err = ErrorPayload.FromMap(em);

        var sc =
            d.TryGetValue(WireKey.StreamControl, out var scRaw) && scRaw is string scs
                ? StreamControlExt.FromWire(scs)
                : (StreamControl?)null;
        var seq = d.TryGetValue(WireKey.Seq, out var seqRaw)
            ? seqRaw switch
            {
                long l when l >= 0 => (ulong)l,
                int i when i >= 0 => (ulong)i,
                ulong u => u,
                _ => (ulong?)null,
            }
            : null;

        return new ResponseEnvelope
        {
            Id = (string)d[WireKey.Id]!,
            Ok = (bool)d[WireKey.Ok]!,
            Result = d.TryGetValue(WireKey.Result, out var res) ? res : null,
            Error = err,
            Seq = seq,
            StreamControlValue = sc,
        };
    }
}

// ErrorPayload

/// <summary>Wire error detail carried inside a failed ResponseEnvelope.</summary>
public sealed class ErrorPayload
{
    public string Code { get; init; } = "Internal";
    public string Message { get; init; } = "";
    public IReadOnlyDictionary<string, object?> Details { get; init; } =
        new Dictionary<string, object?>();

    public static ErrorPayload FromMap(Dictionary<string, object?> d) =>
        new()
        {
            Code = d.TryGetValue(WireKey.Code, out var c) && c is string cs ? cs : "Internal",
            Message = d.TryGetValue(WireKey.Message, out var m) && m is string ms ? ms : "",
            Details =
                d.TryGetValue(WireKey.Details, out var det) && det is Dictionary<string, object?> dm
                    ? dm
                    : new Dictionary<string, object?>(),
        };
}
