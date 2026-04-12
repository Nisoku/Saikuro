---
title: "C# Adapter Examples"
description: "C#-centered cross-language patterns"
---

## Internal admin service with capabilities

Use C# for internal workflows and enforce capability-gated operations.

```csharp
var provider = new Provider("admin");
provider.Register("purge_queue", async args =>
{
    var queueName = (string)args[0];
    return await PurgeAsync(queueName);
});
await provider.ServeAsync("tcp://127.0.0.1:7700");
```

Caller with capability token:

```csharp
var client = new Client(new ClientOptions { CapabilityToken = token });
await client.ConnectAsync();
var purged = await client.CallAsync("admin.purge_queue", new object[] { "dead-letter" });
```

## WASM/browser client

In Blazor WASM, use WebSocket transport.

```csharp
var client = new Client(new WebSocketTransport("ws://localhost:7700"));
await client.ConnectAsync();
```

## Next Steps

- [C# Adapter](./)
- [Transports](../../guide/transports)
- [Examples hub](../../guide/examples)