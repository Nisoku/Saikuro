---
type: concept
title: "C# API Reference"
description: "Full C# adapter API reference"
source: "https://nisoku.org/Saikuro/adapters/csharp/api-reference/"
path: /adapters/csharp/api-reference/
updated: 2026-06-27
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-06-27T13:25:19.823Z"
---
---
title: "C# API Reference"
description: "Full C# adapter API reference"
---

See the [C# adapter overview](./) for the complete API surface.

## Client

| Method                       | Signature                                                              | Description                   |
|------------------------------|------------------------------------------------------------------------|-------------------------------|
| `ConnectAsync`               | `(string address) -> Task`                                             | Connect to runtime at address |
| `CloseAsync`                 | `() -> Task`                                                           | Gracefully close              |
| `CallAsync<T>`               | `(string target, object[] args, CancellationToken?) -> Task<T>`        | Request/response              |
| `CastAsync`                  | `(string target, object[] args) -> Task`                               | Fire-and-forget               |
| `StreamAsync<T>`             | `(string target, object[] args) -> IAsyncEnumerable<T>`                | Open a stream                 |
| `ChannelAsync<TSend, TRecv>` | `(string target, object[] args) -> Task<SaikuroChannel<TSend, TRecv>>` | Open a channel                |
| `BatchAsync`                 | `((string, object[])[] calls) -> Task<object[]>`                       | Batch multiple calls          |
| `ResourceAsync`              | `(string target, object[] args) -> Task<ResourceHandle>`               | Resource handle               |
| `LogAsync`                   | `(string level, string name, string msg, object? fields) -> Task`      | Structured log                |

## Provider

| Method                 | Signature                                                           | Description         |
|------------------------|---------------------------------------------------------------------|---------------------|
| `Register<T, TResult>` | `(string name, Func<T, TResult> handler, FunctionOptions?) -> void` | Register a function |
| `RegisterStream<T>`    | `(string name, Func<T, IAsyncEnumerable<TResult>> handler) -> void` | Register stream     |
| `ServeAsync`           | `(string address) -> Task`                                          | Serve on address    |
