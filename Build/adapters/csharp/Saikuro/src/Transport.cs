// Saikuro transport abstraction.
//
// Implementations:
//   - InMemoryTransport:  in-process, paired queues (testing) [always available]
//   - TcpTransport:  TCP, 4-byte big-endian length-prefix framing [native only]
//   - UnixSocketTransport:  Unix domain socket, same framing [native/unix only]
//   - WebSocketTransport:  WebSocket, no length prefix [always available]
//
// All implement ITransport and accept/emit plain Dictionary<string,object?> maps
// that map directly onto the wire MessagePack representation.
//
// WASM compatibility:
//   InMemoryTransport and WebSocketTransport work on all platforms including Blazor WASM.
//   TcpTransport and UnixSocketTransport are excluded when compiling for wasm.

using System.Buffers.Binary;
using System.Net;
using System.Net.WebSockets;
using System.Threading.Channels;
using MessagePack;
#if !WASM
using System.Net.Sockets;
#endif

namespace Saikuro;

//  Serialisation helpers

internal static class MsgpackHelper
{
    // ContractlessStandardResolver handles Dictionary<string,object?> directly.
    private static readonly MessagePackSerializerOptions Options =
        MessagePackSerializerOptions.Standard.WithResolver(
            MessagePack.Resolvers.ContractlessStandardResolver.Instance
        );

    internal static byte[] Encode(Dictionary<string, object?> obj) =>
        MessagePackSerializer.Serialize(obj, Options);

    internal static Dictionary<string, object?> Decode(ReadOnlyMemory<byte> bytes) =>
        MessagePackSerializer.Deserialize<Dictionary<string, object?>>(bytes, Options)
        ?? throw new InvalidDataException("MessagePack decoded to null");
}

//  Interface

/// <summary>
/// All Saikuro transports expose this interface.
/// Messages are plain <c>Dictionary&lt;string, object?&gt;</c> maps that
/// correspond 1-to-1 with the on-wire MessagePack representation.
/// </summary>
public interface ITransport
{
    /// <summary>Establish the connection.</summary>
    Task ConnectAsync(CancellationToken ct = default);

    /// <summary>Close the connection gracefully.</summary>
    Task CloseAsync(CancellationToken ct = default);

    /// <summary>Serialise <paramref name="obj"/> and transmit it.</summary>
    Task SendAsync(Dictionary<string, object?> obj, CancellationToken ct = default);

    /// <summary>
    /// Receive the next message.
    /// Returns <c>null</c> on clean EOF (peer closed).
    /// </summary>
    Task<Dictionary<string, object?>?> RecvAsync(CancellationToken ct = default);
}

//  Frame codec

internal static class FrameCodec
{
    internal const int MaxFrameSize = 16 * 1024 * 1024; // 16 MiB

    /// <summary>Write a length-prefixed frame to <paramref name="stream"/>.</summary>
    internal static async Task WriteFrameAsync(Stream stream, byte[] payload, CancellationToken ct)
    {
        if (payload.Length > MaxFrameSize)
            throw new InvalidOperationException(
                $"Frame {payload.Length} bytes exceeds maximum {MaxFrameSize}."
            );
        var header = new byte[4];
        BinaryPrimitives.WriteUInt32BigEndian(header, (uint)payload.Length);
        await stream.WriteAsync(header, ct).ConfigureAwait(false);
        await stream.WriteAsync(payload, ct).ConfigureAwait(false);
        await stream.FlushAsync(ct).ConfigureAwait(false);
    }

    /// <summary>
    /// Read one length-prefixed frame from <paramref name="stream"/>.
    /// Returns <c>null</c> on clean EOF.
    /// </summary>
    internal static async Task<byte[]?> ReadFrameAsync(Stream stream, CancellationToken ct)
    {
        var header = new byte[4];
        var read = await ReadExactAsync(stream, header, ct).ConfigureAwait(false);
        if (read == 0)
            return null; // clean EOF

        var length = (int)BinaryPrimitives.ReadUInt32BigEndian(header);
        if (length > MaxFrameSize)
            throw new InvalidDataException(
                $"Incoming frame claims {length} bytes:  exceeds maximum {MaxFrameSize}."
            );

        if (length == 0)
            return Array.Empty<byte>();

        var payload = new byte[length];
        var got = await ReadExactAsync(stream, payload, ct).ConfigureAwait(false);
        if (got < length)
            throw new EndOfStreamException("Connection closed mid-frame.");
        return payload;
    }

    private static async Task<int> ReadExactAsync(Stream stream, byte[] buf, CancellationToken ct)
    {
        int offset = 0;
        while (offset < buf.Length)
        {
            var n = await stream.ReadAsync(buf.AsMemory(offset), ct).ConfigureAwait(false);
            if (n == 0)
                return offset; // EOF
            offset += n;
        }
        return offset;
    }
}

//  InMemoryTransport

/// <summary>
/// In-process transport backed by <see cref="Channel{T}"/> queues.
/// Create a connected pair with <see cref="Pair"/>.
/// </summary>
public sealed class InMemoryTransport : ITransport
{
    private readonly Channel<Dictionary<string, object?>> _sendCh;
    private readonly Channel<Dictionary<string, object?>> _recvCh;
    private bool _closed;

    private InMemoryTransport(
        Channel<Dictionary<string, object?>> sendCh,
        Channel<Dictionary<string, object?>> recvCh
    )
    {
        _sendCh = sendCh;
        _recvCh = recvCh;
    }

    /// <summary>Create a connected pair of in-memory transports.</summary>
    public static (InMemoryTransport A, InMemoryTransport B) Pair()
    {
        var chA = Channel.CreateUnbounded<Dictionary<string, object?>>();
        var chB = Channel.CreateUnbounded<Dictionary<string, object?>>();
        return (new InMemoryTransport(chA, chB), new InMemoryTransport(chB, chA));
    }

    public Task ConnectAsync(CancellationToken ct = default) => Task.CompletedTask;

    public Task CloseAsync(CancellationToken ct = default)
    {
        if (!_closed)
        {
            _closed = true;
            // Complete the send channel so the peer's RecvAsync sees EOF.
            _sendCh.Writer.TryComplete();
        }
        return Task.CompletedTask;
    }

    public async Task SendAsync(Dictionary<string, object?> obj, CancellationToken ct = default)
    {
        if (_closed)
            throw new InvalidOperationException("InMemoryTransport: transport is closed.");
        await _sendCh.Writer.WriteAsync(obj, ct).ConfigureAwait(false);
    }

    public async Task<Dictionary<string, object?>?> RecvAsync(CancellationToken ct = default)
    {
        try
        {
            // Use WaitToReadAsync + TryRead instead of ReadAsync so we handle the
            // completed channel (EOF) path without relying on exceptions.  When
            // the writer is completed and there are no more items,
            // WaitToReadAsync returns false which we treat as EOF (null).
            while (await _recvCh.Reader.WaitToReadAsync(ct).ConfigureAwait(false))
            {
                if (_recvCh.Reader.TryRead(out var item))
                    return item;
            }

            // Writer completed and no items left: clean EOF.
            return null;
        }
        catch (OperationCanceledException)
        {
            return null;
        }
    }
}

#if !WASM
//  Base for TCP / Unix socket transports

/// <summary>
/// Shared plumbing for transports backed by a <see cref="NetworkStream"/>
/// with 4-byte big-endian length-prefix framing. Only <see cref="ConnectAsync"/>
/// and resource cleanup differ between TCP and Unix domain sockets.
/// </summary>
public abstract class StreamTransport : ITransport
{
    protected NetworkStream? _stream;

    public abstract Task ConnectAsync(CancellationToken ct = default);

    /// <summary>Release any resources beyond <see cref="_stream"/>.</summary>
    protected abstract void Cleanup();

    public async Task CloseAsync(CancellationToken ct = default)
    {
        if (_stream is not null)
        {
            await _stream.DisposeAsync().ConfigureAwait(false);
            _stream = null;
        }
        Cleanup();
    }

    public async Task SendAsync(Dictionary<string, object?> obj, CancellationToken ct = default)
    {
        EnsureConnected();
        var payload = MsgpackHelper.Encode(obj);
        await FrameCodec.WriteFrameAsync(_stream!, payload, ct).ConfigureAwait(false);
    }

    public async Task<Dictionary<string, object?>?> RecvAsync(CancellationToken ct = default)
    {
        EnsureConnected();
        var payload = await FrameCodec.ReadFrameAsync(_stream!, ct).ConfigureAwait(false);
        if (payload is null)
            return null;
        return MsgpackHelper.Decode(payload);
    }

    private void EnsureConnected()
    {
        if (_stream is null)
            throw new InvalidOperationException("StreamTransport: not connected.");
    }
}

//  TcpTransport

/// <summary>TCP transport with 4-byte big-endian length-prefix framing.
/// Not available on WebAssembly (Blazor).</summary>
public sealed class TcpTransport : StreamTransport
{
    private readonly string _host;
    private readonly int _port;
    private TcpClient? _client;

    public TcpTransport(string host, int port)
    {
        _host = host;
        _port = port;
    }

    public override async Task ConnectAsync(CancellationToken ct = default)
    {
        _client = new TcpClient();
        await _client.ConnectAsync(_host, _port, ct).ConfigureAwait(false);
        _stream = _client.GetStream();
    }

    protected override void Cleanup()
    {
        _client?.Dispose();
        _client = null;
    }
}

//  UnixSocketTransport

/// <summary>Unix domain socket transport with 4-byte big-endian length-prefix framing.
/// Not available on WebAssembly (Blazor).</summary>
public sealed class UnixSocketTransport : StreamTransport
{
    private readonly string _path;
    private Socket? _socket;

    public UnixSocketTransport(string path) => _path = path;

    public override async Task ConnectAsync(CancellationToken ct = default)
    {
        _socket = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
        var ep = new UnixDomainSocketEndPoint(_path);
        await _socket.ConnectAsync(ep, ct).ConfigureAwait(false);
        _stream = new NetworkStream(_socket, ownsSocket: false);
    }

    protected override void Cleanup()
    {
        _socket?.Dispose();
        _socket = null;
    }
}
#endif

//  WebSocketTransport

/// <summary>
/// WebSocket transport (no length prefix:  WS is already message-framed).
/// </summary>
public sealed class WebSocketTransport : ITransport
{
    private readonly Uri _uri;
    private ClientWebSocket? _ws;

    public WebSocketTransport(string uri) => _uri = new Uri(uri);

    public async Task ConnectAsync(CancellationToken ct = default)
    {
        _ws = new ClientWebSocket();
        await _ws.ConnectAsync(_uri, ct).ConfigureAwait(false);
    }

    public async Task CloseAsync(CancellationToken ct = default)
    {
        if (_ws is { State: WebSocketState.Open })
            await _ws.CloseAsync(WebSocketCloseStatus.NormalClosure, "close", ct)
                .ConfigureAwait(false);
        _ws?.Dispose();
        _ws = null;
    }

    public async Task SendAsync(Dictionary<string, object?> obj, CancellationToken ct = default)
    {
        EnsureConnected();
        var payload = MsgpackHelper.Encode(obj);
        if (payload.Length > FrameCodec.MaxFrameSize)
            throw new InvalidOperationException(
                $"Frame {payload.Length} bytes exceeds maximum {FrameCodec.MaxFrameSize}."
            );
        await _ws!
            .SendAsync(payload, WebSocketMessageType.Binary, endOfMessage: true, ct)
            .ConfigureAwait(false);
    }

    public async Task<Dictionary<string, object?>?> RecvAsync(CancellationToken ct = default)
    {
        if (_ws is null || _ws.State != WebSocketState.Open)
            return null;

        try
        {
            // Accumulate the message across potentially multiple WS fragments.
            var buf = new byte[4096];
            using var ms = new System.IO.MemoryStream();
            WebSocketReceiveResult result;
            do
            {
                result = await _ws.ReceiveAsync(buf, ct).ConfigureAwait(false);
                if (result.MessageType == WebSocketMessageType.Close)
                    return null;
                ms.Write(buf, 0, result.Count);
            } while (!result.EndOfMessage);

            return MsgpackHelper.Decode(ms.ToArray());
        }
        catch (OperationCanceledException)
        {
            return null;
        }
        catch (WebSocketException)
        {
            return null;
        }
    }

    private void EnsureConnected()
    {
        if (_ws is null || _ws.State != WebSocketState.Open)
            throw new InvalidOperationException("WebSocketTransport: not connected.");
    }
}

//  Factory

/// <summary>
/// Construct the appropriate transport for an address string.
///
/// Formats:
///   unix:///path/to/socket  (native only, not available on WASM)
///   tcp://host:port         (native only, not available on WASM)
///   ws://host:port/path     (available on all platforms including WASM)
///   wss://host:port/path    (available on all platforms including WASM)
///
/// Note: On WebAssembly (Blazor), only ws:// and wss:// schemes are supported.
/// Attempting to use tcp:// or unix:// on WASM will throw ArgumentException.
/// </summary>
public static class TransportFactory
{
    public static ITransport MakeTransport(string address)
    {
#if WASM
        // On WASM, only WebSocket transport is available
        if (
            address.StartsWith("ws://", StringComparison.Ordinal)
            || address.StartsWith("wss://", StringComparison.Ordinal)
        )
            return new WebSocketTransport(address);

        if (
            address.StartsWith("tcp://", StringComparison.Ordinal)
            || address.StartsWith("unix://", StringComparison.Ordinal)
        )
            throw new ArgumentException(
                $"Transport scheme not supported on WebAssembly: \"{address}\". "
                    + "Only ws:// and wss:// are available on WASM."
            );

        throw new ArgumentException(
            $"Unsupported transport address: \"{address}\". "
                + "Supported schemes on WASM: ws://, wss://"
        );
#else
        // On native, all transports are available
        if (address.StartsWith("unix://", StringComparison.Ordinal))
            return new UnixSocketTransport(address["unix://".Length..]);

        if (address.StartsWith("tcp://", StringComparison.Ordinal))
        {
            var rest = address["tcp://".Length..];
            var lastColon = rest.LastIndexOf(':');
            var host = rest[..lastColon];
            var port = int.Parse(rest[(lastColon + 1)..]);
            return new TcpTransport(host, port);
        }

        if (
            address.StartsWith("ws://", StringComparison.Ordinal)
            || address.StartsWith("wss://", StringComparison.Ordinal)
        )
            return new WebSocketTransport(address);

        throw new ArgumentException(
            $"Unsupported transport address: \"{address}\". "
                + "Supported schemes: unix://, tcp://, ws://, wss://"
        );
#endif
    }
}
