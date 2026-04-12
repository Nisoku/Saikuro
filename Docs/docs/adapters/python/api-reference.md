---
title: "Python API Reference"
description: "Reference for the Saikuro Python adapter"
---

This page is the Python adapter API reference. For shared protocol details, see [Core Protocol Reference](../../api/).

## Exported core types

- `SaikuroClient`
- `SaikuroProvider`
- `SaikuroStream`
- `SaikuroChannel`
- `SaikuroError`
- `InMemoryTransport`

## SaikuroClient construction and lifecycle

| Symbol | Signature | Returns | Notes |
| ------ | --------- | ------- | ----- |
| `SaikuroClient.connect` | `@classmethod async connect(address: str)` | `SaikuroClient` | Creates transport and connects. |
| `SaikuroClient.from_transport` | `@classmethod from_transport(transport)` | `SaikuroClient` | Uses existing transport. |
| `client.close` | `async close()` | `None` | Closes connection and pending ops. |

## SaikuroClient invocation API

| Symbol | Signature | Returns | Notes |
| ------ | --------- | ------- | ----- |
| `client.call` | `async call(target, args, capability=None, timeout=None)` | `Any` | Request/response invocation. |
| `client.cast` | `async cast(target, args, capability=None)` | `None` | Fire-and-forget invocation. |
| `client.batch` | `async batch(calls, timeout=None)` | `List[Any]` | Ordered per-call results. |
| `client.resource` | `async resource(target, args, capability=None, timeout=None)` | `ResourceHandle` | Resource-handle invocation. |
| `client.stream` | `async stream(target, args, capability=None)` | `SaikuroStream` | Server-to-client stream. |
| `client.channel` | `async channel(target, args, capability=None)` | `SaikuroChannel` | Bidirectional channel. |
| `client.log` | `async log(level, name, msg, fields=None)` | `None` | Structured runtime logging. |

## SaikuroProvider API

| Symbol | Signature | Returns | Notes |
| ------ | --------- | ------- | ----- |
| `new SaikuroProvider` | `SaikuroProvider(namespace: str)` | `SaikuroProvider` | Namespace-scoped provider. |
| `provider.register` | `register(name, capabilities=None, doc=None)` | `Callable[[Handler], Handler]` | Decorator-based registration. |
| `provider.register_function` | `register_function(name, fn, capabilities=None, doc=None)` | `None` | Imperative registration. |
| `provider.schema_dict` | `schema_dict()` | `dict` | Build announcement schema. |
| `provider.serve` | `async serve(address: str)` | `None` | Connect and serve loop. |
| `provider.serve_on_transport` | `async serve_on_transport(transport)` | `None` | Serve on existing transport. |

## Error model

Errors raise `SaikuroError` with `code` and `message` fields.

## Related

- [Python Adapter Overview](./)
- [Python Examples](./examples)
- [Core Protocol Reference](../../api/)