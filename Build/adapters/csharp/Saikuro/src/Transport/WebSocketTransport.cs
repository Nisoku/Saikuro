using System.Net.WebSockets;

namespace Saikuro;

public sealed class WebSocketTransport : ITransport
{
    private const int WebSocketBufferSize = 8192; // 8 KiB chunk for incremental reads

    private readonly Uri _uri;
    private ClientWebSocket? _ws;
    private int _closed;

    public WebSocketTransport(string uri) => _uri = new Uri(uri);

    public async Task ConnectAsync(CancellationToken ct = default)
    {
        _ws = new ClientWebSocket();
        await _ws.ConnectAsync(_uri, ct).ConfigureAwait(false);
    }

    public async Task CloseAsync(CancellationToken ct = default)
    {
        if (Interlocked.Exchange(ref _closed, 1) != 0)
            return;
        if (_ws is { State: WebSocketState.Open })
            await _ws.CloseAsync(WebSocketCloseStatus.NormalClosure, "close", ct)
                .ConfigureAwait(false);
        _ws?.Dispose();
        _ws = null;
    }

    public async Task SendAsync(Dictionary<string, object?> obj, CancellationToken ct = default)
    {
        ObjectDisposedException.ThrowIf(Volatile.Read(ref _closed) != 0, this);
        var ws = _ws;
        if (ws is null || ws.State != WebSocketState.Open)
            throw new InvalidOperationException("WebSocketTransport: not connected.");
        var payload = MsgpackHelper.Encode(obj);
        if (payload.Length > FrameCodec.MaxFrameSize)
            throw new InvalidOperationException(
                $"Frame {payload.Length} bytes exceeds maximum {FrameCodec.MaxFrameSize}."
            );
        await ws
            .SendAsync(payload, WebSocketMessageType.Binary, endOfMessage: true, ct)
            .ConfigureAwait(false);
    }

    public async Task<Dictionary<string, object?>?> RecvAsync(CancellationToken ct = default)
    {
        ObjectDisposedException.ThrowIf(Volatile.Read(ref _closed) != 0, this);
        var ws = _ws;
        if (ws is null || ws.State != WebSocketState.Open)
            return null;

        try
        {
            var buf = new byte[WebSocketBufferSize];
            using var ms = new System.IO.MemoryStream();
            WebSocketReceiveResult result;
            do
            {
                result = await ws.ReceiveAsync(buf, ct).ConfigureAwait(false);
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
}
