using System.Threading.Channels;

namespace Saikuro;

public sealed class InMemoryTransport : ITransport
{
    private readonly Channel<Dictionary<string, object?>> _sendCh;
    private readonly Channel<Dictionary<string, object?>> _recvCh;
    private int _closed;

    private InMemoryTransport(
        Channel<Dictionary<string, object?>> sendCh,
        Channel<Dictionary<string, object?>> recvCh
    )
    {
        _sendCh = sendCh;
        _recvCh = recvCh;
    }

    public static (InMemoryTransport A, InMemoryTransport B) Pair()
    {
        var chA = Channel.CreateUnbounded<Dictionary<string, object?>>();
        var chB = Channel.CreateUnbounded<Dictionary<string, object?>>();
        return (new InMemoryTransport(chA, chB), new InMemoryTransport(chB, chA));
    }

    public Task ConnectAsync(CancellationToken ct = default) => Task.CompletedTask;

    public Task CloseAsync(CancellationToken ct = default)
    {
        if (Interlocked.Exchange(ref _closed, 1) == 0)
        {
            _recvCh.Writer.TryComplete();
            _sendCh.Writer.TryComplete();
        }
        return Task.CompletedTask;
    }

    public async Task SendAsync(Dictionary<string, object?> obj, CancellationToken ct = default)
    {
        ObjectDisposedException.ThrowIf(Volatile.Read(ref _closed) != 0, this);
        await _sendCh.Writer.WriteAsync(obj, ct).ConfigureAwait(false);
    }

    public async Task<Dictionary<string, object?>?> RecvAsync(CancellationToken ct = default)
    {
        ObjectDisposedException.ThrowIf(Volatile.Read(ref _closed) != 0, this);

        if (ct.IsCancellationRequested)
            return null;

        while (await _recvCh.Reader.WaitToReadAsync(ct).ConfigureAwait(false))
        {
            if (_recvCh.Reader.TryRead(out var item))
                return item;
        }

        return null;
    }
}
