#if NET8_0_OR_GREATER
namespace Saikuro
{
    internal sealed class SaikuroRuntimeConnection : IAsyncDisposable
    {
        private readonly string _connId;
        private readonly CancellationTokenSource _cts = new();
        private bool _closed;

        public string ConnectionId => _connId;

        private SaikuroRuntimeConnection(string connId)
        {
            _connId = connId;
        }

        public static async Task<SaikuroRuntimeConnection> ConnectAsync(
            string channelName, CancellationToken ct = default)
        {
            var connId = await BroadcastChannelInterop.ConnectToRuntime(channelName)
                .WaitAsync(ct);
            return new SaikuroRuntimeConnection(connId);
        }

        public async Task SendAsync(byte[] data, CancellationToken ct = default)
        {
            ObjectDisposedException.ThrowIf(_closed, this);
            BroadcastChannelInterop.SendRuntime(_connId, data);
            await Task.CompletedTask;
        }

        public async Task<byte[]?> RecvAsync(CancellationToken ct = default)
        {
            using var linkedCts = CancellationTokenSource.CreateLinkedTokenSource(ct, _cts.Token);
            try
            {
                // Wait for the signal: the JS function checks the queue first
                // (returns true synchronously), or returns a Promise<true> that
                // resolves when the next BroadcastChannel message arrives via
                // onmessage.
                await BroadcastChannelInterop
                    .WaitForRuntimeMessage(_connId)
                    .WaitAsync(linkedCts.Token);
                return BroadcastChannelInterop.DequeueRuntimeMessage(_connId);
            }
            catch (OperationCanceledException)
            {
                return null;
            }
        }

        public async Task CloseAsync()
        {
            if (_closed) return;
            _closed = true;
            _cts.Cancel();
            _cts.Dispose();
            BroadcastChannelInterop.CloseRuntime(_connId);
            await Task.CompletedTask;
        }

        public async ValueTask DisposeAsync()
        {
            await CloseAsync();
        }
    }
}
#endif