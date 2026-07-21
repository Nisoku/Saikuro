---
type: concept
title: "C# Examples"
description: "C# adapter usage patterns"
source: "https://nisoku.org/Saikuro/docs/adapters/csharp/examples/"
path: /adapters/csharp/examples/
updated: 2026-07-21
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-07-21T10:55:59.274Z"
---
---
title: "C# Examples"
description: "C# adapter usage patterns"
---

## Streaming Provider

```csharp
var provider = new Provider("events");

// Use IAsyncEnumerable for streaming
provider.RegisterStream("subscribe", async (string topic) =>
{
    await using var subscriber = await SubscribeAsync(topic);
    await foreach (var evt in subscriber)
    {
        yield return evt;
    }
});

await provider.ServeAsync("unix:///tmp/saikuro.sock");
```

## Client with Logging

```csharp
var client = new Client();
await client.ConnectAsync("tcp://10.0.0.5:7700");

// Structured logging
await client.LogAsync("info", "myapp", "started", new { version = "1.0" });

// With timeout
using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(5));
var result = await client.CallAsync<int>("math.add", new object[] { 1, 2 }, cts.Token);
```
