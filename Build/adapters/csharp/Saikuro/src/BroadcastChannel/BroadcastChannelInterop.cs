#if NET8_0_OR_GREATER
using System.Runtime.InteropServices.JavaScript;
namespace Saikuro
{
    internal static partial class BroadcastChannelInterop
    {
        [JSImport("globalThis.Saikuro_CreateBC")]
        public static partial string CreateBC(string name);

        [JSImport("globalThis.Saikuro_PostMessage")]
        public static partial void PostMessage(string channelId, byte[] data);

        [JSImport("globalThis.Saikuro_CloseBC")]
        public static partial void CloseBC(string channelId);

        [JSImport("globalThis.Saikuro_ConnectToRuntime")]
        public static partial Task<string> ConnectToRuntime(string baseName);

        [JSImport("globalThis.Saikuro_WaitForRuntimeMessage")]
        public static partial Task<bool> WaitForRuntimeMessage(string connId);

        [JSImport("globalThis.Saikuro_DequeueRuntimeMessage")]
        public static partial byte[]? DequeueRuntimeMessage(string connId);

        [JSImport("globalThis.Saikuro_SendRuntime")]
        public static partial void SendRuntime(string connId, byte[] data);

        [JSImport("globalThis.Saikuro_CloseRuntime")]
        public static partial void CloseRuntime(string connId);
    }
}
#endif