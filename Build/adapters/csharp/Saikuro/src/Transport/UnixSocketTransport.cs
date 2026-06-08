#if !WASM
using System.Net.Sockets;

namespace Saikuro;

/// <summary>Unix domain socket transport with 4-byte big-endian length-prefix framing.
/// Not available on WebAssembly (Blazor).</summary>
public sealed class UnixSocketTransport : StreamTransport
{
    private readonly string _path;
    private Socket? _socket;

    public UnixSocketTransport(string path) => _path = path;

    public override async Task ConnectAsync(CancellationToken ct = default)
    {
        if (_socket is not null || _stream is not null)
            await CloseAsync(ct).ConfigureAwait(false);
        var socket = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
        try
        {
            var ep = new UnixDomainSocketEndPoint(_path);
            await socket.ConnectAsync(ep, ct).ConfigureAwait(false);
            _socket = socket;
            _stream = new NetworkStream(_socket, ownsSocket: false);
        }
        catch
        {
            socket.Dispose();
            throw;
        }
    }

    protected override void Cleanup()
    {
        _socket?.Dispose();
        _socket = null;
    }
}
#endif
