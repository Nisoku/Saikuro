namespace Saikuro;

/// <summary>
/// All Saikuro transports expose this interface.
/// Messages are plain <c>Dictionary&lt;string, object?&gt;</c> maps that
/// correspond 1-to-1 with the on-wire MessagePack representation.
/// </summary>
public interface ITransport
{
    /// <summary>Establish the connection.</summary>
    Task ConnectAsync(CancellationToken ct = default);

    /// <summary>Close the connection gracefully.</summary>
    Task CloseAsync(CancellationToken ct = default);

    /// <summary>Serialise <paramref name="obj"/> and transmit it.</summary>
    Task SendAsync(Dictionary<string, object?> obj, CancellationToken ct = default);

    /// <summary>
    /// Receive the next message.
    /// Returns <c>null</c> on clean EOF (peer closed).
    /// </summary>
    Task<Dictionary<string, object?>?> RecvAsync(CancellationToken ct = default);
}
