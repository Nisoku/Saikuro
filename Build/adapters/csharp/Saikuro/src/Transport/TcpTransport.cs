#if !WASM
using System.Net.Sockets;

namespace Saikuro;

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
        if (_client is not null || _stream is not null)
            await CloseAsync(ct).ConfigureAwait(false);
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
#endif
