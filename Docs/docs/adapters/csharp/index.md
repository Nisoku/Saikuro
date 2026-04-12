---
title: "C# Adapter"
description: "Saikuro adapter for .NET"
---

The C# adapter targets `net8.0`, `netstandard2.0`, and `netstandard2.1`.

## Install

```bash
dotnet add package Saikuro
```

## Basic usage

```csharp
using Saikuro;

await using var client = await Client.ConnectAsync("tcp://127.0.0.1:7700");
var result = await client.CallAsync("math.add", new object[] { 1, 2 });
Console.WriteLine(result);
```

## Provider pattern

```csharp
using Saikuro;

var provider = new Provider("math");
provider.Register("add", async args =>
{
    var a = Convert.ToInt64(args[0]);
    var b = Convert.ToInt64(args[1]);
    return (object)(a + b);
});
await provider.ServeAsync("tcp://127.0.0.1:7700");
```

## WASM and Blazor

In browser/WASM targets, use `WebSocketTransport` or `InMemoryTransport`. TCP and Unix socket transports require a native target.

## Schema extractor

A reflection-based extractor tool is included in:

- `Build/adapters/csharp/tools/extractor`

## Next Steps

- [Transports](../../guide/transports): Transport options and behavior
- [C# API Reference](./api-reference): .NET adapter method reference
- [C# examples](./examples): Cross-language integration patterns