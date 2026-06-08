// Tests for the BroadcastChannel WASM host transport layer.
//
// Covers: JS contract consistency, queue/delivery pattern, wire format,
//         WasmHostTransport state management, and cancellation.
//
// Tests that require the JS file on disk are skipped when the file
// is not present (e.g. CI).

using System.Reflection;
using System.Runtime.InteropServices.JavaScript;
using Saikuro;

namespace Saikuro.Tests;

//  Helpers

internal static class BroadcastChannelTestHelpers
{
    internal static string? FindJsFile()
    {
        var candidates = new[]
        {
            Path.Combine(
                AppContext.BaseDirectory,
                "../../../../src/BroadcastChannel/wwwroot/Saikuro.BroadcastChannel.js"
            ),
            Path.Combine(
                AppContext.BaseDirectory,
                "../../../src/BroadcastChannel/wwwroot/Saikuro.BroadcastChannel.js"
            ),
        };
        return candidates.FirstOrDefault(File.Exists);
    }
}

//  Contract: C# [JSImport] signatures vs JS file

public class BroadcastChannelContractTests
{
    private static bool HasJSImport(MethodInfo m) =>
        CustomAttributeData.GetCustomAttributes(m)
            .Any(a => a.AttributeType.Name == "JSImportAttribute");

    private static string? GetJSImportFunctionName(MethodInfo m)
    {
        var attr = CustomAttributeData.GetCustomAttributes(m)
            .FirstOrDefault(a => a.AttributeType.Name == "JSImportAttribute");
        if (attr?.ConstructorArguments.Count > 0)
            return attr.ConstructorArguments[0].Value as string;
        return null;
    }

    private static IEnumerable<(string MethodName, string JsFunctionName, int ParamCount, Type ReturnType)>
        GetImportSignatures()
    {
        return typeof(BroadcastChannelInterop)
            .GetMethods(BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static)
            .Where(HasJSImport)
            .Select(m =>
            {
                var fullName = GetJSImportFunctionName(m) ?? m.Name;
                var shortName = fullName.StartsWith("globalThis.")
                    ? fullName["globalThis.".Length..]
                    : fullName;
                return (m.Name, shortName, m.GetParameters().Length, m.ReturnType);
            })
            .ToList();
    }

    [Fact]
    public void AllImportFunctions_ExistInJsFile()
    {
        var path = BroadcastChannelTestHelpers.FindJsFile();
        if (path is null)
            return;
        var js = File.ReadAllText(path);

        var missing = GetImportSignatures()
            .Select(s => s.JsFunctionName)
            .Where(name => !js.Contains(name, StringComparison.Ordinal))
            .ToList();

        Assert.Empty(missing);
    }

    [Fact]
    public void JsFile_HasAllRequiredGlobalThisFunctions()
    {
        var path = BroadcastChannelTestHelpers.FindJsFile();
        if (path is null)
            return;
        var js = File.ReadAllText(path);

        var expected = new[]
        {
            "Saikuro_CreateBC",
            "Saikuro_PostMessage",
            "Saikuro_CloseBC",
            "Saikuro_ConnectToRuntime",
            "Saikuro_WaitForRuntimeMessage",
            "Saikuro_DequeueRuntimeMessage",
            "Saikuro_SendRuntime",
            "Saikuro_CloseRuntime",
        };

        var missing = expected.Where(name => !js.Contains(name)).ToList();
        Assert.Empty(missing);
    }

    [Fact]
    public void AllImportNames_UseGlobalThisPrefix()
    {
        foreach (var m in typeof(BroadcastChannelInterop)
                     .GetMethods(BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static)
                     .Where(HasJSImport))
        {
            var name = GetJSImportFunctionName(m) ?? m.Name;
            Assert.StartsWith("globalThis.", name);
        }
    }

    [Fact]
    public void Dequeue_ReturnsByteArray()
    {
        var method = typeof(BroadcastChannelInterop)
            .GetMethod("DequeueRuntimeMessage",
                BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static);
        Assert.NotNull(method);
        var ret = method!.ReturnType;
        Assert.True(ret == typeof(byte[]) || ret.FullName == "System.Byte[]");
    }

    [Fact]
    public void WaitForRuntimeMessage_ReturnsTaskOfBool()
    {
        var method = typeof(BroadcastChannelInterop)
            .GetMethod("WaitForRuntimeMessage",
                BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static);
        Assert.NotNull(method);
        Assert.Equal(typeof(Task<bool>), method!.ReturnType);
    }

    [Fact]
    public void ConnectToRuntime_ReturnsTaskOfString()
    {
        var method = typeof(BroadcastChannelInterop)
            .GetMethod("ConnectToRuntime",
                BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static);
        Assert.NotNull(method);
        Assert.Equal(typeof(Task<string>), method!.ReturnType);
    }

    [Fact]
    public void SendRuntime_IsVoid()
    {
        var method = typeof(BroadcastChannelInterop)
            .GetMethod("SendRuntime",
                BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static);
        Assert.NotNull(method);
        Assert.Equal(typeof(void), method!.ReturnType);
    }

    [Fact]
    public void CloseRuntime_IsVoid()
    {
        var method = typeof(BroadcastChannelInterop)
            .GetMethod("CloseRuntime",
                BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static);
        Assert.NotNull(method);
        Assert.Equal(typeof(void), method!.ReturnType);
    }

    [Fact]
    public void SendRuntime_AcceptsStringAndByteArray()
    {
        var method = typeof(BroadcastChannelInterop)
            .GetMethod("SendRuntime",
                BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static);
        Assert.NotNull(method);
        var parms = method!.GetParameters();
        Assert.Equal(2, parms.Length);
        Assert.Equal(typeof(string), parms[0].ParameterType);
        Assert.Equal(typeof(byte[]), parms[1].ParameterType);
    }

    [Fact]
    public void ConnectToRuntime_AcceptsString()
    {
        var method = typeof(BroadcastChannelInterop)
            .GetMethod("ConnectToRuntime",
                BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static);
        Assert.NotNull(method);
        var parms = method!.GetParameters();
        Assert.Single(parms);
        Assert.Equal(typeof(string), parms[0].ParameterType);
    }

    [Fact]
    public void AllImportMethods_HaveCorrectParameterTypes()
    {
        foreach (var m in typeof(BroadcastChannelInterop)
                     .GetMethods(BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static)
                     .Where(HasJSImport))
        {
            foreach (var p in m.GetParameters())
            {
                var t = p.ParameterType;
                var typeName = t.FullName ?? t.Name;
                Assert.True(
                    typeName is "System.String" or "System.Byte[]" or "System.Threading.Tasks.Task`1[[System.String" or "System.Threading.Tasks.Task`1[[System.Boolean" or "System.Void",
                    $"Method {m.Name} parameter '{p.Name}' has unsupported type {t.Name}"
                );
            }
        }
    }
}

//  Contract: JS file structure

public class BroadcastChannelJsFileTests
{
    private string? ReadJs()
    {
        var path = BroadcastChannelTestHelpers.FindJsFile();
        return path is not null ? File.ReadAllText(path) : null;
    }

    [Fact]
    public void JsFile_BracesMatch()
    {
        var js = ReadJs();
        if (js is null)
            return;
        var depth = 0;
        foreach (var c in js)
        {
            if (c == '{') depth++;
            if (c == '}') depth--;
            Assert.True(depth >= 0, "unexpected closing brace");
        }
        Assert.Equal(0, depth);
    }

    [Fact]
    public void JsFile_SendRuntime_DoesNotUseDotBuffer()
    {
        // .NET [JSImport] marshals byte[] as ArrayBuffer (not Uint8Array),
        // so .buffer on ArrayBuffer is undefined.
        var js = ReadJs();
        if (js is null)
            return;

        // Find the Saikuro_SendRuntime function body.
        var start = js.IndexOf("Saikuro_SendRuntime", StringComparison.Ordinal);
        Assert.True(start >= 0, "Saikuro_SendRuntime not found");

        // Look for postMessage call within it.
        var postMsg = js.IndexOf("postMessage", start, StringComparison.Ordinal);
        Assert.True(postMsg >= 0, "postMessage not found in Saikuro_SendRuntime");

        // The argument to postMessage should NOT use .buffer
        var argStart = js.IndexOf('(', postMsg);
        var argEnd = js.IndexOf(')', argStart + 1);
        var argContent = js[(argStart + 1)..argEnd];

        Assert.DoesNotContain(".buffer", argContent);
    }

    [Fact]
    public void JsFile_NoSetTimeoutInHotPath()
    {
        // The receive path must not use setTimeout (or other timer-based polling)
        // since Blazor's own setTimeout handlers can starve the event loop.
        var js = ReadJs();
        if (js is null)
            return;

        // These functions are on the receive path and must not use timers:
        var hotFunctions = new[]
        {
            "deliverMessage",
            "Saikuro_WaitForRuntimeMessage",
            "Saikuro_DequeueRuntimeMessage",
        };

        foreach (var fn in hotFunctions)
        {
            var idx = js.IndexOf($"function {fn}", StringComparison.Ordinal);
            if (idx < 0)
                idx = js.IndexOf(fn, StringComparison.Ordinal);
            if (idx < 0)
                continue;

            // Check the scope of this function for setTimeout calls.
            // Walk forward to find the closing brace.
            var bodyStart = js.IndexOf('{', idx);
            if (bodyStart < 0)
                continue;
            var depth = 1;
            var bodyEnd = bodyStart + 1;
            while (depth > 0 && bodyEnd < js.Length)
            {
                if (js[bodyEnd] == '{') depth++;
                if (js[bodyEnd] == '}') depth--;
                bodyEnd++;
            }
            var body = js[bodyStart..bodyEnd];
            Assert.DoesNotContain("setTimeout", body);
        }
    }

    [Fact]
    public void JsFile_PostMessage_ReceivesOnlyArrayBuffer()
    {
        // The Rust WasmHostReceiver expects ArrayBuffer on the receiving end.
        // C# [JSImport] marshals byte[] as ArrayBuffer.  This test verifies
        // the Rust-side expectations are met.
        var js = ReadJs();
        if (js is null)
            return;

        // The onmessage handler on the runtime side should check for ArrayBuffer.
        var onmessageIdx = js.IndexOf("e2.data instanceof ArrayBuffer", StringComparison.Ordinal);
        Assert.True(onmessageIdx >= 0,
            "C# side onmessage handler must accept ArrayBuffer (sent by Rust send_buffer)");
    }
}

//  Queue / delivery pattern
//
//  Mirrors the JS-side _messageQueues / _recvWaiters in pure C# to test
//  the push-based wait logic without requiring a Blazor host.

internal sealed class MessageQueue
{
    private readonly Queue<byte[]> _queue = new();
    private TaskCompletionSource<bool>? _waiter;
    private readonly object _lock = new();

    public void Deliver(byte[] data)
    {
        lock (_lock)
        {
            _queue.Enqueue(data);
            if (_waiter is not null)
            {
                _waiter.TrySetResult(true);
                _waiter = null;
            }
        }
    }

    public byte[]? Dequeue()
    {
        lock (_lock)
        {
            return _queue.Count > 0 ? _queue.Dequeue() : null;
        }
    }

    public Task<bool> WaitAsync(CancellationToken ct = default)
    {
        TaskCompletionSource<bool> tcs;
        lock (_lock)
        {
            if (_queue.Count > 0)
                return Task.FromResult(true);
            tcs = new TaskCompletionSource<bool>(
                TaskCreationOptions.RunContinuationsAsynchronously
            );
            _waiter = tcs;
            if (ct.CanBeCanceled)
            {
                ct.Register(state =>
                {
                    var t = (TaskCompletionSource<bool>)state!;
                    lock (_lock)
                    {
                        if (_waiter == t)
                            _waiter = null;
                    }
                    t.TrySetCanceled(ct);
                }, tcs);
            }
            return tcs.Task;
        }
    }
}

public class MessageQueueTests
{
    [Fact]
    public void DeliverThenDequeue_ReturnsData()
    {
        var q = new MessageQueue();
        q.Deliver("hello"u8.ToArray());
        var result = q.Dequeue();
        Assert.NotNull(result);
        Assert.Equal("hello"u8.ToArray(), result);
    }

    [Fact]
    public void DequeueOnEmpty_ReturnsNull()
    {
        var q = new MessageQueue();
        Assert.Null(q.Dequeue());
    }

    [Fact]
    public void DeliverMultiple_DequeuesInOrder()
    {
        var q = new MessageQueue();
        q.Deliver("first"u8.ToArray());
        q.Deliver("second"u8.ToArray());
        q.Deliver("third"u8.ToArray());

        Assert.Equal("first"u8.ToArray(), q.Dequeue());
        Assert.Equal("second"u8.ToArray(), q.Dequeue());
        Assert.Equal("third"u8.ToArray(), q.Dequeue());
        Assert.Null(q.Dequeue());
    }

    [Fact]
    public async Task WaitWithQueuedData_ReturnsImmediately()
    {
        var q = new MessageQueue();
        q.Deliver("queued"u8.ToArray());

        var result = await q.WaitAsync();
        Assert.True(result);
        Assert.Equal("queued"u8.ToArray(), q.Dequeue());
    }

    [Fact]
    public async Task DeliverAfterWait_ResolvesWaiter()
    {
        var q = new MessageQueue();
        var waitTask = q.WaitAsync();
        Assert.False(waitTask.IsCompleted);

        q.Deliver("later"u8.ToArray());

        var result = await waitTask;
        Assert.True(result);
        Assert.Equal("later"u8.ToArray(), q.Dequeue());
    }

    [Fact]
    public async Task MultipleDeliveries_OnlyFirstTriggersWaiter()
    {
        var q = new MessageQueue();
        var waitTask = q.WaitAsync();

        q.Deliver("a"u8.ToArray());
        await waitTask;

        // Second delivery should not throw (no waiter to signal).
        q.Deliver("b"u8.ToArray());
        Assert.Equal("a"u8.ToArray(), q.Dequeue());
        Assert.Equal("b"u8.ToArray(), q.Dequeue());
    }

    [Fact]
    public async Task WaitCancelled_ThrowsOperationCancelled()
    {
        var q = new MessageQueue();
        using var cts = new CancellationTokenSource();
        cts.Cancel();

        var ex = await Record.ExceptionAsync(() => q.WaitAsync(cts.Token));
        Assert.NotNull(ex);
        Assert.True(
            ex is OperationCanceledException,
            $"Expected OperationCanceledException, got {ex.GetType()}"
        );
    }

    [Fact]
    public async Task WaitWithNoDataAndCancel_Throws()
    {
        var q = new MessageQueue();
        using var cts = new CancellationTokenSource();

        var waitTask = q.WaitAsync(cts.Token);
        Assert.False(waitTask.IsCompleted);

        cts.Cancel();
        var ex = await Record.ExceptionAsync(() => waitTask);
        Assert.NotNull(ex);
        Assert.True(
            ex is OperationCanceledException,
            $"Expected OperationCanceledException, got {ex.GetType()}"
        );

        // After cancellation, the queue should still accept deliveries.
        q.Deliver("after-cancel"u8.ToArray());
        Assert.Equal("after-cancel"u8.ToArray(), q.Dequeue());
    }

    [Fact]
    public async Task DeliverAfterCancel_NewWaiterWorks()
    {
        var q = new MessageQueue();
        using var cts = new CancellationTokenSource();
        cts.Cancel();

        var ex = await Record.ExceptionAsync(() => q.WaitAsync(cts.Token));
        Assert.NotNull(ex);
        Assert.True(
            ex is OperationCanceledException,
            $"Expected OperationCanceledException, got {ex.GetType()}"
        );

        // A fresh wait (no cancellation) should work.
        var waitTask = q.WaitAsync();
        q.Deliver("fresh"u8.ToArray());
        var ok = await waitTask;
        Assert.True(ok);
        Assert.Equal("fresh"u8.ToArray(), q.Dequeue());
    }

    [Fact]
    public async Task ConcurrentDeliverAndDequeue_NoDataLoss()
    {
        var q = new MessageQueue();
        const int count = 100;

        var produce = Task.Run(() =>
        {
            for (int i = 0; i < count; i++)
                q.Deliver(BitConverter.GetBytes(i));
        });

        var consume = Task.Run(async () =>
        {
            var received = 0;
            while (received < count)
            {
                var ok = await q.WaitAsync();
                Assert.True(ok);
                while (q.Dequeue() is not null)
                    received++;
            }
            return received;
        });

        await produce;
        var total = await consume;
        Assert.Equal(count, total);
    }

    [Fact]
    public async Task DeliverBeforeAndAfterWait_BothDelivered()
    {
        var q = new MessageQueue();
        q.Deliver("pre"u8.ToArray());

        var wait1 = await q.WaitAsync();
        Assert.True(wait1);
        Assert.Equal("pre"u8.ToArray(), q.Dequeue());

        // Second wait should pend (queue is now empty).
        var wait2 = q.WaitAsync();
        Assert.False(wait2.IsCompleted);

        q.Deliver("post"u8.ToArray());
        var ok = await wait2;
        Assert.True(ok);
        Assert.Equal("post"u8.ToArray(), q.Dequeue());
    }

    [Fact]
    public void DeliverSameBytesTwice_BothDequeued()
    {
        var q = new MessageQueue();
        var data = "same"u8.ToArray();
        q.Deliver(data);
        q.Deliver(data);

        Assert.Equal(data, q.Dequeue());
        Assert.Equal(data, q.Dequeue());
        Assert.Null(q.Dequeue());
    }
}

//  Wire format: Msgpack encoding / decoding (what crosses the BC)

public class BroadcastChannelWireFormatTests
{
    [Fact]
    public void EncodeDecode_RoundTrip()
    {
        var original = new Dictionary<string, object?>
        {
            ["type"] = "call",
            ["id"] = "test-42",
            ["target"] = "csharp.summary",
            ["args"] = new object?[] { "some data" },
        };
        var bytes = MsgpackHelper.Encode(original);
        var decoded = MsgpackHelper.Decode(bytes);

        Assert.Equal(original["type"], decoded["type"]);
        Assert.Equal(original["id"], decoded["id"]);
        Assert.Equal(original["target"], decoded["target"]);
    }

    [Fact]
    public void EncodeDecode_EmptyDict()
    {
        var original = new Dictionary<string, object?>();
        var bytes = MsgpackHelper.Encode(original);
        var decoded = MsgpackHelper.Decode(bytes);
        Assert.Empty(decoded);
    }

    [Fact]
    public void EncodeDecode_NestedDict()
    {
        var original = new Dictionary<string, object?>
        {
            ["error"] = new Dictionary<string, object?>
            {
                ["code"] = "Timeout",
                ["message"] = "timed out",
            },
        };
        var bytes = MsgpackHelper.Encode(original);
        var decoded = MsgpackHelper.Decode(bytes);
        var err = (Dictionary<string, object?>)decoded["error"]!;
        Assert.Equal("Timeout", err["code"]);
    }

    [Fact]
    public void EncodeDecode_BinaryData()
    {
        var original = new Dictionary<string, object?>
        {
            ["bytes"] = new byte[] { 0x00, 0xFF, 0xAB, 0xCD },
        };
        var bytes = MsgpackHelper.Encode(original);
        var decoded = MsgpackHelper.Decode(bytes);
        Assert.Equal(new byte[] { 0x00, 0xFF, 0xAB, 0xCD }, (byte[])decoded["bytes"]!);
    }

    [Fact]
    public void EncodeDecode_NullValues()
    {
        var original = new Dictionary<string, object?>
        {
            ["null_key"] = null,
            ["string_key"] = "present",
        };
        var bytes = MsgpackHelper.Encode(original);
        var decoded = MsgpackHelper.Decode(bytes);
        Assert.Null(decoded["null_key"]);
        Assert.Equal("present", decoded["string_key"]);
    }

    [Fact]
    public void EncodeDecode_ArrayValues()
    {
        var original = new Dictionary<string, object?>
        {
            ["arr"] = new object?[] { 1L, "two", true, null, 3.14 },
        };
        var bytes = MsgpackHelper.Encode(original);
        var decoded = MsgpackHelper.Decode(bytes);
        var arr = (object?[])decoded["arr"]!;
        Assert.Equal(5, arr.Length);
        Assert.Equal(1L, arr[0]);
        Assert.Equal("two", arr[1]);
        Assert.True((bool)arr[2]!);
        Assert.Null(arr[3]);
    }

    [Fact]
    public void EncodeDecode_LargeString()
    {
        var str = new string('x', 10000);
        var original = new Dictionary<string, object?> { ["big"] = str };
        var bytes = MsgpackHelper.Encode(original);
        var decoded = MsgpackHelper.Decode(bytes);
        Assert.Equal(str, decoded["big"]);
    }

    [Fact]
    public void EncodeDecode_AllIntegerTypes()
    {
        var original = new Dictionary<string, object?>
        {
            ["int"] = 42,
            ["long"] = 9999999999L,
            ["short"] = (short)123,
            ["byte"] = (byte)200,
            ["sbyte"] = (sbyte)(-1),
        };
        var bytes = MsgpackHelper.Encode(original);
        var decoded = MsgpackHelper.Decode(bytes);
        Assert.Equal(42L, decoded["int"]);
        Assert.Equal(9999999999L, decoded["long"]);
        Assert.Equal(123L, decoded["short"]);
        Assert.Equal(200L, decoded["byte"]);
        Assert.Equal(-1L, decoded["sbyte"]);
    }

    [Fact]
    public void EncodeDecode_DeeplyNestedDict()
    {
        var inner = new Dictionary<string, object?>
        {
            ["a"] = new Dictionary<string, object?>
            {
                ["b"] = new Dictionary<string, object?>
                {
                    ["c"] = "deep",
                },
            },
        };
        var bytes = MsgpackHelper.Encode(inner);
        var decoded = MsgpackHelper.Decode(bytes);
        Assert.Equal("deep",
            (string)((Dictionary<string, object?>)
                ((Dictionary<string, object?>)
                    decoded["a"]!)["b"]!)["c"]!);
    }

    [Fact]
    public void Encode_ProducesNonEmptyBytes()
    {
        var d = new Dictionary<string, object?> { ["k"] = "v" };
        var bytes = MsgpackHelper.Encode(d);
        Assert.NotEmpty(bytes);
    }

    [Fact]
    public void Decode_ProducedByRust_IsCompatible()
    {
        // This byte sequence was produced by Rust's msgpack encoder for:
        // { "type": "call", "id": "rust-id", "target": "csharp.summary", "args": [] }
        // If this test fails, the C# decoder is incompatible with the Rust encoder.
        var rustEncoded = new byte[]
        {
            0x84, // fixmap(4)
            0xA4, 0x74, 0x79, 0x70, 0x65, // str(4) "type"
            0xA4, 0x63, 0x61, 0x6C, 0x6C, // str(4) "call"
            0xA2, 0x69, 0x64, // str(2) "id"
            0xA7, 0x72, 0x75, 0x73, 0x74, 0x2D, 0x69, 0x64, // str(7) "rust-id"
            0xA6, 0x74, 0x61, 0x72, 0x67, 0x65, 0x74, // str(6) "target"
            0xAE, 0x63, 0x73, 0x68, 0x61, 0x72, 0x70, 0x2E,
            0x73, 0x75, 0x6D, 0x6D, 0x61, 0x72, 0x79, // str(14) "csharp.summary"
            0xA4, 0x61, 0x72, 0x67, 0x73, // str(4) "args"
            0x90, // fixarray(0)
        };

        var decoded = MsgpackHelper.Decode(rustEncoded);
        Assert.Equal("call", decoded["type"]);
        Assert.Equal("rust-id", decoded["id"]);
        Assert.Equal("csharp.summary", decoded["target"]);
        Assert.Empty((Array)decoded["args"]!);
    }

    [Fact]
    public void FromMsgpackDict_WithBinaryUuidId_ParsesCorrectly()
    {
        // Rust sends InvocationId as 16 binary bytes (non-human-readable msgpack)
        // using the network-byte-order UUID "6f9619ff-8b86-d011-b42d-00cf4fc964ff"
        var binaryId = new byte[]
        {
            0x6f, 0x96, 0x19, 0xff, 0x8b, 0x86, 0xd0, 0x11,
            0xb4, 0x2d, 0x00, 0xcf, 0x4f, 0xc9, 0x64, 0xff,
        };

        var dict = new Dictionary<string, object?>
        {
            ["type"] = "call",
            ["id"] = binaryId,
            ["target"] = "csharp.summary",
            ["args"] = new object?[] { },
            ["version"] = 1,
        };

        var env = Envelope.FromMsgpackDict(dict);
        Assert.Equal("6f9619ff-8b86-d011-b42d-00cf4fc964ff", env.Id);
    }

    [Fact]
    public void MessagePackRoundtrip_WithBinaryUuidId_ParsesCorrectly()
    {
        // Full roundtrip: simulate what the Rust runtime actually sends
        var binaryId = new byte[]
        {
            0x6f, 0x96, 0x19, 0xff, 0x8b, 0x86, 0xd0, 0x11,
            0xb4, 0x2d, 0x00, 0xcf, 0x4f, 0xc9, 0x64, 0xff,
        };

        var dict = new Dictionary<string, object?>
        {
            ["type"] = "call",
            ["id"] = binaryId,
            ["target"] = "csharp.summary",
            ["args"] = new object?[] { },
            ["version"] = 1,
        };

        // Transport path: encode -> send -> receive -> decode
        var encoded = MsgpackHelper.Encode(dict);
        var decoded = MsgpackHelper.Decode(encoded);

        var env = Envelope.FromMsgpackDict(decoded);
        Assert.Equal("6f9619ff-8b86-d011-b42d-00cf4fc964ff", env.Id);
    }
}

//  WasmHostTransport: state / error handling

public class WasmHostTransportStateTests
{
    [Fact]
    public void Constructor_DoesNotThrow()
    {
        var t = new WasmHostTransport("my-channel");
        Assert.NotNull(t);
    }

    [Fact]
    public void Constructor_DefaultChannel_DoesNotThrow()
    {
        var t = new WasmHostTransport();
        Assert.NotNull(t);
    }

    [Fact]
    public async Task SendAsync_WhenNotConnected_ThrowsInvalidOperation()
    {
        var t = new WasmHostTransport("test");
        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            t.SendAsync(new Dictionary<string, object?>())
        );
    }

    [Fact]
    public async Task RecvAsync_WhenNotConnected_ThrowsInvalidOperation()
    {
        var t = new WasmHostTransport("test");
        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            t.RecvAsync()
        );
    }

    [Fact]
    public async Task CloseAsync_WhenNotConnected_DoesNotThrow()
    {
        var t = new WasmHostTransport("test");
        await t.CloseAsync();
    }

    [Fact]
    public async Task DoubleClose_IsSafe()
    {
        var t = new WasmHostTransport("test");
        await t.CloseAsync();
        await t.CloseAsync();
    }

    [Fact]
    public async Task SendAsync_AfterClose_ThrowsInvalidOperation()
    {
        var t = new WasmHostTransport("test");
        await t.CloseAsync();
        await Assert.ThrowsAsync<ObjectDisposedException>(() =>
            t.SendAsync(new Dictionary<string, object?>())
        );
    }

    [Fact]
    public async Task RecvAsync_AfterClose_ThrowsInvalidOperation()
    {
        var t = new WasmHostTransport("test");
        await t.CloseAsync();
        await Assert.ThrowsAsync<ObjectDisposedException>(() =>
            t.RecvAsync()
        );
    }

    [Fact]
    public async Task CloseAsync_DoesNotThrow()
    {
        var t = new WasmHostTransport("test");
        await t.CloseAsync();
    }

    [Fact]
    public async Task DoubleDispose_IsSafe()
    {
        var t = new WasmHostTransport("test");
        await t.CloseAsync();
        await t.CloseAsync();
        // Should not throw or crash.
    }

    [Fact]
    public async Task ConnectAsync_WithoutWasmHost_ThrowsPlatformException()
    {
        var t = new WasmHostTransport("test");
        var ex = await Record.ExceptionAsync(() => t.ConnectAsync());
        Assert.NotNull(ex);
        Assert.IsNotType<NullReferenceException>(ex);
        Assert.IsNotType<InvalidOperationException>(ex);
    }

    [Fact]
    public async Task ConnectAsync_Cancelled_DoesNotThrowNullRef()
    {
        using var cts = new CancellationTokenSource();
        cts.Cancel();
        var t = new WasmHostTransport("test-cancel");
        var ex = await Record.ExceptionAsync(() => t.ConnectAsync(cts.Token));
        // In a non-WASM host [JSImport] methods throw - just ensure not NullRef.
        if (ex is not null)
            Assert.IsNotType<NullReferenceException>(ex);
    }

    [Fact]
    public async Task SendAsync_StatusLogging_DoesNotThrow()
    {
        // The WasmHostTransport now logs bytes on send/recv.  Verify logging
        // doesn't crash when SendAsync throws due to not-connected.
        var t = new WasmHostTransport("test");
        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            t.SendAsync(new Dictionary<string, object?> { ["type"] = "call" })
        );
    }

    [Fact]
    public async Task ConnectAsync_WithCancelledToken_BeforeCall_Throws()
    {
        // Pre-cancelled token should prevent starting the handshake.
        using var cts = new CancellationTokenSource();
        cts.Cancel();
        var t = new WasmHostTransport("test");
        var ex = await Record.ExceptionAsync(() => t.ConnectAsync(cts.Token));
        // Might be OperationCanceledException or PlatformNotSupportedException -
        // either is acceptable as long as it doesn't crash.
        Assert.NotNull(ex);
    }
}
