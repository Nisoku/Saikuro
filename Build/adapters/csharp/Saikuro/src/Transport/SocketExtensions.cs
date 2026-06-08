using System.Net;
using System.Net.Sockets;

namespace Saikuro;

internal static class SocketExtensions
{
    public static Task ConnectAsync(this Socket socket, EndPoint remoteEp, CancellationToken ct)
    {
#if NET5_0_OR_GREATER
        return socket.ConnectAsync(remoteEp, ct).AsTask();
#else
        if (ct.IsCancellationRequested)
            return Task.FromCanceled(ct);

        var tcs = new TaskCompletionSource<object>(TaskCreationOptions.RunContinuationsAsynchronously);
        var args = new SocketAsyncEventArgs { RemoteEndPoint = remoteEp };
        args.Completed += (_, _) =>
        {
            if (args.SocketError == SocketError.Success)
                tcs.TrySetResult(null!);
            else
                tcs.TrySetException(new SocketException((int)args.SocketError));
            args.Dispose();
        };
        var reg = ct.Register(() => tcs.TrySetCanceled(ct));
        if (!socket.ConnectAsync(args))
        {
            if (args.SocketError == SocketError.Success)
                tcs.TrySetResult(null!);
            else
                tcs.TrySetException(new SocketException((int)args.SocketError));
            args.Dispose();
        }
        tcs.Task.ContinueWith(_ => reg.Dispose(), TaskScheduler.Default);
        return tcs.Task;
#endif
    }
}

internal static class TcpClientExtensions
{
    public static Task ConnectAsync(this TcpClient client, string host, int port, CancellationToken ct)
    {
#if NET5_0_OR_GREATER
        return client.ConnectAsync(host, port, ct).AsTask();
#else
        if (ct.IsCancellationRequested)
            return Task.FromCanceled(ct);

        var tcs = new TaskCompletionSource<object>(TaskCreationOptions.RunContinuationsAsynchronously);
        var reg = ct.Register(() => tcs.TrySetCanceled(ct));
        client.BeginConnect(host, port, ar =>
        {
            try
            {
                client.EndConnect(ar);
                tcs.TrySetResult(null!);
            }
            catch (Exception ex)
            {
                tcs.TrySetException(ex);
            }
        }, null);
        tcs.Task.ContinueWith(_ => reg.Dispose(), TaskScheduler.Default);
        return tcs.Task;
#endif
    }
}
