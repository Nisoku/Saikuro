#if NET8_0_OR_GREATER
namespace Saikuro;

public sealed partial class WasmHostTransport : ITransport
{
    private static readonly SaikuroLogger Log = SaikuroLogger.GetLogger("saikuro.transport.wasmhost");

    private readonly string _channelName;
    private SaikuroRuntimeConnection? _connection;
    private bool _closed;

    public WasmHostTransport(string channelName = "saikuro")
    {
        _channelName = channelName;
    }

    public async Task ConnectAsync(CancellationToken ct = default)
    {
        _connection = await SaikuroRuntimeConnection.ConnectAsync(_channelName, ct);
    }

    public async Task SendAsync(Dictionary<string, object?> obj, CancellationToken ct = default)
    {
        ObjectDisposedException.ThrowIf(_closed, this);
        if (_connection is null)
            throw new InvalidOperationException("WasmHostTransport: not connected.");
        var data = MsgpackHelper.Encode(obj);
        Log.Debug("wasmhost send", new Dictionary<string, object?>
        {
            ["bytes"] = data.Length,
            ["type"] = obj.TryGetValue("type", out var t) ? t?.ToString() ?? "?" : "?",
        });
        await _connection.SendAsync(data, ct);
    }

    public async Task<Dictionary<string, object?>?> RecvAsync(CancellationToken ct = default)
    {
        ObjectDisposedException.ThrowIf(_closed, this);
        if (_connection is null)
            throw new InvalidOperationException("WasmHostTransport: not connected.");
        var data = await _connection.RecvAsync(ct);
        if (data == null) return null;
        Log.Debug("wasmhost recv", new Dictionary<string, object?>
        {
            ["bytes"] = data.Length,
        });
        return MsgpackHelper.Decode(data);
    }

    public async Task CloseAsync(CancellationToken ct = default)
    {
        if (_closed) return;
        _closed = true;
        if (_connection != null)
        {
            await _connection.CloseAsync();
            _connection = null;
        }
    }
}
#endif
