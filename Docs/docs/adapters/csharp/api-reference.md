---
title: "C# API Reference"
description: "Reference for the Saikuro .NET adapter"
---

This page is the C# adapter API reference. For shared protocol details, see [Core Protocol Reference](../../api/).

## Main types

- `SaikuroClient`
- `SaikuroProvider`
- `SaikuroStream<T>`
- `SaikuroChannel<TIn, TOut>`
- `SaikuroException`
- `ITransport`, `InMemoryTransport`, `TcpTransport`, `UnixSocketTransport`, `WebSocketTransport`

## SaikuroClient factory and lifecycle

| Symbol | Signature | Returns | Notes |
| ------ | --------- | ------- | ----- |
| `SaikuroClient.ConnectAsync` | `public static async Task<SaikuroClient> ConnectAsync(address, options?, ct?)` | `Task<SaikuroClient>` | Creates transport from address and connects. |
| `SaikuroClient.OpenOnAsync` | `public static async Task<SaikuroClient> OpenOnAsync(transport, options?, ct?)` | `Task<SaikuroClient>` | Uses existing transport instance. |
| `SaikuroClient.FromTransport` | `public static SaikuroClient FromTransport(transport, options?)` | `SaikuroClient` | Does not auto-connect. |
| `client.OpenAsync` | `public async Task OpenAsync(ct?)` | `Task` | Starts receive loop. |
| `client.CloseAsync` | `public async Task CloseAsync(ct?)` | `Task` | Graceful shutdown. |
| `client.Connected` | `public bool Connected` | `bool` | Connection state. |

## SaikuroClient invocation API

| Symbol | Signature | Returns | Notes |
| ------ | --------- | ------- | ----- |
| `client.CallAsync` | `public async Task<object?> CallAsync(target, args, capability?, timeout?, ct?)` | `Task<object?>` | Request/response invocation. |
| `client.CastAsync` | `public Task CastAsync(target, args, capability?, ct?)` | `Task` | Fire-and-forget invocation. |
| `client.ResourceAsync` | `public async Task<ResourceHandle> ResourceAsync(target, args, capability?, timeout?, ct?)` | `Task<ResourceHandle>` | Resource-handle invocation. |
| `client.BatchAsync` | `public async Task<IReadOnlyList<object?>> BatchAsync(calls, timeout?, ct?)` | `Task<IReadOnlyList<object?>>` | Ordered per-call results. |
| `client.StreamAsync<T>` | `public async Task<SaikuroStream<T>> StreamAsync<T>(target, args, capability?, ct?)` | `Task<SaikuroStream<T>>` | Server-to-client stream. |
| `client.ChannelAsync<TIn, TOut>` | `public async Task<SaikuroChannel<TIn, TOut>> ChannelAsync<TIn, TOut>(target, args, capability?, ct?)` | `Task<SaikuroChannel<TIn, TOut>>` | Bidirectional channel. |

## SaikuroProvider API

| Symbol | Signature | Returns | Notes |
| ------ | --------- | ------- | ----- |
| `new SaikuroProvider` | `public SaikuroProvider(string namespace)` | `SaikuroProvider` | Namespace-scoped provider. |
| `provider.Register` | `public SaikuroProvider Register(...)` | `SaikuroProvider` | Overloads for sync, async, and stream handlers. |
| `provider.SchemaDict` | `public Dictionary<string, object?> SchemaDict()` | `Dictionary<string, object?>` | Schema announcement payload. |
| `provider.DispatchAsync` | `public async Task DispatchAsync(...)` | `Task` | Low-level dispatch path. |
| `provider.ServeAsync` | `public async Task ServeAsync(address, ct?)` | `Task` | Connect and serve loop. |
| `provider.ServeOnAsync` | `public async Task ServeOnAsync(transport, ct?)` | `Task` | Serve on existing transport. |

## Error model

Errors surface as `SaikuroException` with protocol-aligned error codes.

## Related

- [C# Adapter Overview](./)
- [C# Examples](./examples)
- [Core Protocol Reference](../../api/)