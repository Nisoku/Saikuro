using System.Buffers.Binary;

namespace Saikuro;

internal static class FrameCodec
{
    /// <summary>Maximum frame size: 16 MiB. Matches common runtime limits (e.g. WASM, gRPC).</summary>
    public const int MaxFrameSize = 16 * 1024 * 1024;

    /// <summary>Header size: 4-byte big-endian length prefix.</summary>
    public const int HeaderSize = 4;

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
        if (read < 4)
            throw new EndOfStreamException("Connection closed mid-header.");

        var rawLength = BinaryPrimitives.ReadUInt32BigEndian(header);
        if (rawLength > MaxFrameSize)
            throw new InvalidDataException(
                $"Incoming frame claims {rawLength} bytes: exceeds maximum {MaxFrameSize}."
            );
        var length = (int)rawLength;

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
