#if !WASM
using System.Net.Sockets;

namespace Saikuro;

/// <summary>
/// Shared plumbing for transports backed by a <see cref="NetworkStream"/>
/// with 4-byte big-endian length-prefix framing. Only <see cref="ConnectAsync"/>
/// and resource cleanup differ between TCP and Unix domain sockets.
/// </summary>
public abstract class StreamTransport : ITransport
{
    protected NetworkStream? _stream;
    private int _closed;

    public abstract Task ConnectAsync(CancellationToken ct = default);

    /// <summary>Release any resources beyond <see cref="_stream"/>.</summary>
    protected abstract void Cleanup();

    public async Task CloseAsync(CancellationToken ct = default)
    {
        if (Interlocked.Exchange(ref _closed, 1) != 0)
            return;
        try
        {
            if (_stream is not null)
                await _stream.DisposeAsync().ConfigureAwait(false);
        }
        finally
        {
            _stream = null;
            Cleanup();
        }
    }

    public async Task SendAsync(Dictionary<string, object?> obj, CancellationToken ct = default)
    {
        ObjectDisposedException.ThrowIf(Volatile.Read(ref _closed) != 0, this);
        EnsureConnected();
        var payload = MsgpackHelper.Encode(obj);
        await FrameCodec.WriteFrameAsync(_stream!, payload, ct).ConfigureAwait(false);
    }

    public async Task<Dictionary<string, object?>?> RecvAsync(CancellationToken ct = default)
    {
        ObjectDisposedException.ThrowIf(Volatile.Read(ref _closed) != 0, this);
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
#endif
