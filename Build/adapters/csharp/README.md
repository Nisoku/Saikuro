# Saikuro C# adapter

C# adapter for the [Saikuro](https://github.com/Nisoku/Saikuro) cross-language
IPC fabric. Targets `net8.0`, `netstandard2.0`, and `netstandard2.1`, including
Blazor WebAssembly (use `WebSocketTransport` or `InMemoryTransport` in-browser;
TCP and Unix socket require a native target).

## Installation

```bash
dotnet add package Saikuro
```

## Usage

### Client

```csharp
using Saikuro;

await using var client = await Client.ConnectAsync("tcp://127.0.0.1:7700");

var result = await client.CallAsync("math.add", new object[] { 1, 2 });
Console.WriteLine(result); // 3
```

### Provider

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

## Building from source

```bash
cd Saikuro/src
dotnet build

# Run tests:
cd ../..
dotnet test
```

## Schema extractor CLI

The repository includes a C# schema extractor tool for reflection-based schema
generation:

```bash
cd tools/extractor
dotnet run -- parityns
```

The tool emits schema JSON to stdout and is used by parity workflows.

## WASM / Blazor

Compile with the `WASM` preprocessor symbol to exclude TCP and Unix socket
transports:

```bash
dotnet build -p:DefineConstants=WASM
```

## License

Apache-2.0
