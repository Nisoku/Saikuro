namespace Saikuro;

/// <summary>
/// Construct the appropriate transport for an address string.
///
/// Formats:
///   unix:///path/to/socket  (native only, not available on WASM)
///   tcp://host:port         (native only, not available on WASM)
///   ws://host:port/path     (available on all platforms including WASM)
///   wss://host:port/path    (available on all platforms including WASM)
///
/// Note: On WebAssembly (Blazor), only ws:// and wss:// schemes are supported.
/// Attempting to use tcp:// or unix:// on WASM will throw ArgumentException.
/// </summary>
public static class TransportFactory
{
    public static ITransport MakeTransport(string address)
    {
#if WASM
        // On WASM, only WebSocket transport is available
        if (
            address.StartsWith("ws://", StringComparison.Ordinal)
            || address.StartsWith("wss://", StringComparison.Ordinal)
        )
            return new WebSocketTransport(address);

        if (
            address.StartsWith("tcp://", StringComparison.Ordinal)
            || address.StartsWith("unix://", StringComparison.Ordinal)
        )
            throw new ArgumentException(
                $"Transport scheme not supported on WebAssembly: \"{address}\". "
                    + "Only ws:// and wss:// are available on WASM."
            );

        throw new ArgumentException(
            $"Unsupported transport address: \"{address}\". "
                + "Supported schemes on WASM: ws://, wss://"
        );
#else
        // On native, all transports are available
        if (address.StartsWith("unix://", StringComparison.Ordinal))
            return new UnixSocketTransport(address["unix://".Length..]);

        if (address.StartsWith("tcp://", StringComparison.Ordinal))
        {
            var rest = address["tcp://".Length..];
            int colonPos;
            if (rest.StartsWith('['))
            {
                var closeBracket = rest.IndexOf(']');
                if (closeBracket < 0 || closeBracket + 1 >= rest.Length || rest[closeBracket + 1] != ':')
                    throw new ArgumentException($"Invalid TCP address format: \"{address}\"");
                colonPos = closeBracket + 1;
            }
            else
            {
                colonPos = rest.LastIndexOf(':');
            }
            var host = rest[..colonPos];
            var port = int.Parse(rest[(colonPos + 1)..]);
            return new TcpTransport(host, port);
        }

        if (
            address.StartsWith("ws://", StringComparison.Ordinal)
            || address.StartsWith("wss://", StringComparison.Ordinal)
        )
            return new WebSocketTransport(address);

        throw new ArgumentException(
            $"Unsupported transport address: \"{address}\". "
                + "Supported schemes: unix://, tcp://, ws://, wss://"
        );
#endif
    }
}
