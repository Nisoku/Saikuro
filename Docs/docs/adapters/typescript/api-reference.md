---
title: "TypeScript API Reference"
description: "Reference for the Saikuro TypeScript adapter"
---

This page is the TypeScript adapter API reference. For shared protocol details, see [Core Protocol Reference](../../api/).

## Exported core types

- `SaikuroClient`
- `SaikuroProvider`
- `SaikuroStream<T>`
- `SaikuroChannel<TIn, TOut>`
- `SaikuroError`
- `InMemoryTransport`

## SaikuroClient factory and lifecycle

| Symbol | Signature | Returns | Notes |
| ------ | --------- | ------- | ----- |
| `SaikuroClient.connect` | `static async connect(address: string, options?)` | `Promise<SaikuroClient>` | Creates transport from address and connects. |
| `SaikuroClient.openOn` | `static async openOn(transport, options?)` | `Promise<SaikuroClient>` | Uses existing transport instance. |
| `SaikuroClient.fromTransport` | `static fromTransport(transport, options?)` | `SaikuroClient` | Does not auto-connect. |
| `client.open` | `async open()` | `Promise<void>` | Connects transport and starts receive loop. |
| `client.close` | `async close()` | `Promise<void>` | Closes transport and tears down pending ops. |
| `client.connected` | `get connected()` | `boolean` | Connection state. |

## SaikuroClient invocation API

| Symbol | Signature | Returns | Notes |
| ------ | --------- | ------- | ----- |
| `client.call` | `async call(target, args, options?)` | `Promise<unknown>` | Request/response invocation. |
| `client.cast` | `async cast(target, args, options?)` | `Promise<void>` | Fire-and-forget invocation. |
| `client.resource` | `async resource(target, args, options?)` | `Promise<ResourceHandle>` | Resource-handle invocation. |
| `client.batch` | `async batch(calls, options?)` | `Promise<unknown[]>` | Ordered per-call results. |
| `client.stream` | `async stream<T>(target, args, options?)` | `Promise<SaikuroStream<T>>` | Server-to-client stream. |
| `client.channel` | `async channel<TIn, TOut>(target, args, options?)` | `Promise<SaikuroChannel<TIn, TOut>>` | Bidirectional channel. |
| `client.log` | `async log(level, name, msg, fields?)` | `Promise<void>` | Structured runtime logging. |

## SaikuroProvider API

| Symbol | Signature | Returns | Notes |
| ------ | --------- | ------- | ----- |
| `new SaikuroProvider` | `constructor(namespace: string, options?)` | `SaikuroProvider` | Namespace-scoped provider. |
| `provider.register` | `register(name, fn, options?)` | `SaikuroProvider` | Supports sync/async/stream handlers. |
| `provider.serve` | `async serve(address)` | `Promise<void>` | Connects to address and serves loop. |
| `provider.serveOnTransport` | `async serveOnTransport(transport)` | `Promise<void>` | Uses existing transport. |
| `provider.handleEnvelope` | `async handleEnvelope(envelope, transport)` | `Promise<void>` | Low-level dispatch path. |

## Error model

Errors throw as `SaikuroError` with protocol-aligned code/message semantics.

## Related

- [TypeScript Adapter Overview](./)
- [TypeScript Examples](./examples)
- [Core Protocol Reference](../../api/)