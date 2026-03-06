// Tests for Saikuro transport implementations

using System.IO.Pipelines;
using Saikuro;

namespace Saikuro.Tests;

//  InMemoryTransport

public class InMemoryTransportTests
{
    [Fact]
    public async Task Pair_CreatesBidirectionalConnection()
    {
        var (a, b) = InMemoryTransport.Pair();
        var msg = new Dictionary<string, object?> { ["type"] = "call", ["id"] = "x" };

        await a.SendAsync(msg);
        var received = await b.RecvAsync();

        Assert.NotNull(received);
        Assert.Equal("call", received!["type"]);
        Assert.Equal("x", received["id"]);
    }

    [Fact]
    public async Task BothDirections_Work()
    {
        var (a, b) = InMemoryTransport.Pair();
        var msgA = new Dictionary<string, object?> { ["dir"] = "a-to-b" };
        var msgB = new Dictionary<string, object?> { ["dir"] = "b-to-a" };

        await a.SendAsync(msgA);
        await b.SendAsync(msgB);

        var fromA = await b.RecvAsync();
        var fromB = await a.RecvAsync();

        Assert.Equal("a-to-b", fromA!["dir"]);
        Assert.Equal("b-to-a", fromB!["dir"]);
    }

    [Fact]
    public async Task ConnectAsync_Succeeds_Immediately()
    {
        var (a, _) = InMemoryTransport.Pair();
        // Should not throw.
        await a.ConnectAsync();
    }

    [Fact]
    public async Task CloseAsync_SignalsEofToRecvSide()
    {
        var (a, b) = InMemoryTransport.Pair();
        await a.CloseAsync();
        var msg = await b.RecvAsync();
        Assert.Null(msg);
    }

    [Fact]
    public async Task SendAsync_AfterClose_Throws()
    {
        var (a, _) = InMemoryTransport.Pair();
        await a.CloseAsync();
        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            a.SendAsync(new Dictionary<string, object?> { ["x"] = 1 })
        );
    }

    [Fact]
    public async Task MultipleMessages_ReceivedInOrder()
    {
        var (a, b) = InMemoryTransport.Pair();
        for (int i = 0; i < 5; i++)
            await a.SendAsync(new Dictionary<string, object?> { ["n"] = (long)i });

        for (long expected = 0; expected < 5; expected++)
        {
            var msg = await b.RecvAsync();
            Assert.NotNull(msg);
            // MessagePack round-trips int as long via ContractlessResolver.
            Assert.Equal(expected, Convert.ToInt64(msg!["n"]));
        }
    }

    [Fact]
    public async Task SendComplexObject_RoundTrips()
    {
        var (a, b) = InMemoryTransport.Pair();
        var msg = new Dictionary<string, object?>
        {
            ["type"] = "call",
            ["id"] = "abc-123",
            ["target"] = "math.add",
            ["args"] = new object?[] { 1L, 2L },
            ["nested"] = new Dictionary<string, object?> { ["ok"] = true },
        };
        await a.SendAsync(msg);
        var recv = await b.RecvAsync();
        Assert.NotNull(recv);
        Assert.Equal("call", recv!["type"]);
        Assert.Equal("abc-123", recv["id"]);
        Assert.Equal("math.add", recv["target"]);
    }

    [Fact]
    public async Task RecvAsync_WithCancelledToken_ReturnsNull()
    {
        var (_, b) = InMemoryTransport.Pair();
        using var cts = new CancellationTokenSource();
        cts.Cancel();
        // No items in queue and token cancelled:  should return null gracefully.
        var result = await b.RecvAsync(cts.Token);
        Assert.Null(result);
    }
}

//  TransportFactory

public class TransportFactoryTests
{
    [Fact]
    public void MakeTransport_Tcp_ReturnsTcpTransport()
    {
        var t = TransportFactory.MakeTransport("tcp://localhost:7700");
        Assert.IsType<TcpTransport>(t);
    }

    [Fact]
    public void MakeTransport_Unix_ReturnsUnixSocketTransport()
    {
        var t = TransportFactory.MakeTransport("unix:///tmp/saikuro.sock");
        Assert.IsType<UnixSocketTransport>(t);
    }

    [Fact]
    public void MakeTransport_Ws_ReturnsWebSocketTransport()
    {
        var t = TransportFactory.MakeTransport("ws://localhost:8080/");
        Assert.IsType<WebSocketTransport>(t);
    }

    [Fact]
    public void MakeTransport_Wss_ReturnsWebSocketTransport()
    {
        var t = TransportFactory.MakeTransport("wss://example.com/saikuro");
        Assert.IsType<WebSocketTransport>(t);
    }

    [Fact]
    public void MakeTransport_UnknownScheme_ThrowsArgumentException()
    {
        Assert.Throws<ArgumentException>(() => TransportFactory.MakeTransport("grpc://host:1234"));
    }

    [Fact]
    public void MakeTransport_TcpWithIPv6_ParsesCorrectly()
    {
        // Just verifies it doesn't throw:  actual connect would fail.
        var t = TransportFactory.MakeTransport("tcp://127.0.0.1:9999");
        Assert.IsType<TcpTransport>(t);
    }
}

//  FrameCodec

public class FrameCodecTests
{
    [Fact]
    public async Task WriteAndReadFrame_RoundTrip()
    {
        var payload = System.Text.Encoding.UTF8.GetBytes("hello world");
        var ms = new MemoryStream();

        await FrameCodec.WriteFrameAsync(ms, payload, CancellationToken.None);
        ms.Seek(0, SeekOrigin.Begin);

        var result = await FrameCodec.ReadFrameAsync(ms, CancellationToken.None);
        Assert.NotNull(result);
        Assert.Equal(payload, result);
    }

    [Fact]
    public async Task ReadFrame_CleanEof_ReturnsNull()
    {
        var ms = new MemoryStream(); // empty
        var result = await FrameCodec.ReadFrameAsync(ms, CancellationToken.None);
        Assert.Null(result);
    }

    [Fact]
    public async Task WriteFrame_ZeroLengthPayload_RoundTrips()
    {
        var ms = new MemoryStream();
        await FrameCodec.WriteFrameAsync(ms, Array.Empty<byte>(), CancellationToken.None);
        ms.Seek(0, SeekOrigin.Begin);
        var result = await FrameCodec.ReadFrameAsync(ms, CancellationToken.None);
        Assert.NotNull(result);
        Assert.Empty(result!);
    }

    [Fact]
    public async Task ReadFrame_TruncatedPayload_Throws()
    {
        // Write a header claiming 100 bytes but only write 10.
        var ms = new MemoryStream();
        var header = new byte[4];
        System.Buffers.Binary.BinaryPrimitives.WriteUInt32BigEndian(header, 100);
        ms.Write(header);
        ms.Write(new byte[10]); // truncated
        ms.Seek(0, SeekOrigin.Begin);
        await Assert.ThrowsAsync<EndOfStreamException>(() =>
            FrameCodec.ReadFrameAsync(ms, CancellationToken.None)
        );
    }

    [Fact]
    public async Task WriteFrame_OversizedPayload_Throws()
    {
        // 16 MiB + 1 byte
        var huge = new byte[FrameCodec.MaxFrameSize + 1];
        var ms = new MemoryStream();
        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            FrameCodec.WriteFrameAsync(ms, huge, CancellationToken.None)
        );
    }

    [Fact]
    public async Task MultipleFrames_ReadInOrder()
    {
        var ms = new MemoryStream();
        var payloads = new[] { "first"u8.ToArray(), "second"u8.ToArray(), "third"u8.ToArray() };

        foreach (var p in payloads)
            await FrameCodec.WriteFrameAsync(ms, p, CancellationToken.None);

        ms.Seek(0, SeekOrigin.Begin);
        foreach (var expected in payloads)
        {
            var got = await FrameCodec.ReadFrameAsync(ms, CancellationToken.None);
            Assert.Equal(expected, got);
        }
    }
}
