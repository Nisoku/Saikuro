---
type: concept
title: "C# Adapter"
description: "Saikuro adapter for .NET and Blazor WASM"
source: "https://nisoku.org/Saikuro/adapters/csharp/"
path: /adapters/csharp/
updated: 2026-07-09
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-07-09T20:36:11.328Z"
---
---
title: "C# Adapter"
description: "Saikuro adapter for .NET and Blazor WASM"
---

The C# adapter supports .NET 8+ with full async/await and Blazor WASM via BroadcastChannel transport.

## Installation

```bash
dotnet add package Saikuro
```

## Client API

```csharp
using Saikuro;

// Connect via address string
var client = new Client();
await client.ConnectAsync("unix:///tmp/saikuro.sock");

// Call
var result = await client.CallAsync<int>("math.add", new object[] { 1, 2 });

// Cast
await client.CastAsync("log.write", new object[] { new { level = "info", message = "started" } });

// Stream
await foreach (var item in client.StreamAsync<Event>("events.subscribe", Array.Empty<object>()))
{
    Console.WriteLine(item);
}

// Channel
var channel = await client.ChannelAsync<Message, Ack>("chat.session", new object[] { new { room = "general" } });
await channel.SendAsync(new Message { Text = "hello" });
await foreach (var msg in channel)
{
    Console.WriteLine(msg);
}

// Batch
var results = await client.BatchAsync(new[] {
    ("math.add", new object[] { 1, 2 }),
    ("math.multiply", new object[] { 3, 4 }),
});

// Resource
var handle = await client.ResourceAsync("files.open", new object[] { "/data.csv" });

await client.CloseAsync();
```

## Provider API

```csharp
using Saikuro;

var provider = new Provider("math");

provider.Register<int, int, int>("add", (a, b) => a + b);
provider.Register<float, float, float>("divide", (a, b) => {
    if (b == 0) throw new DivideByZeroException();
    return a / b;
}, new FunctionOptions { Capabilities = new[] { "math.divide" } });

await provider.ServeAsync("unix:///tmp/saikuro.sock");
```

## Blazor WASM

In Blazor WASM, use the broadcast channel transport to communicate with the runtime:

```csharp
using Saikuro.Transport;

// The WasmHost transport uses BroadcastChannel internally
var transport = TransportFactory.Create("wasm-host://saikuro");
var client = new Client(transport);
await client.ConnectAsync();
```

## InMemory Testing

```csharp
var (providerTransport, clientTransport) = InMemoryTransport.Pair();

var provider = new Provider("math", providerTransport);
provider.Register<int, int, int>("add", (a, b) => a + b);
await provider.ServeAsync();

var client = new Client(clientTransport);
await client.ConnectAsync();

var result = await client.CallAsync<int>("math.add", new object[] { 1, 2 });
// result == 3
```

## Export Surface

```csharp
// Core
Client, Provider

// Transports
InMemoryTransport, TransportFactory
UnixSocketTransport, TcpTransport, WebSocketTransport
WasmHostTransport

// Extractor
SchemaExtractor
```
