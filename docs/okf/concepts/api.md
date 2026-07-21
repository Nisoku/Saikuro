---
type: api
title: "Protocol & Runtime Reference"
description: "Shared wire format and runtime behavior for all adapters"
source: "https://nisoku.org/Saikuro/docs/api/"
path: /api/
updated: 2026-07-21
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-07-21T10:55:59.281Z"
---
---
title: "Protocol & Runtime Reference"
description: "Shared wire format and runtime behavior for all adapters"
---

Everything Saikuro sends over the wire is a MessagePack-encoded envelope. This page documents the full shape of every message type.

## Encoding

All envelopes use [MessagePack](https://msgpack.org/). You do not need to handle encoding yourself; the adapters do it for you. This reference is for implementing a new adapter or debugging raw traffic.

## Invocation Envelope

Sent by the caller to initiate any invocation. Fields:

```json
{
  "version": 1,
  "type": "call" | "cast" | "stream" | "channel" | "batch" | "resource" | "log" | "announce",
  "id": "<uuid>",
  "target": "namespace.function",
  "args": [...],
  "meta": { ... },
  "capability": "<token>",
  "batch_items": [ ... ],
  "stream_control": "end" | "pause" | "resume" | "abort",
  "seq": 0
}
```

| Field            | Type          | Required | Description                                                                                              |
|------------------|---------------|----------|----------------------------------------------------------------------------------------------------------|
| `version`        | integer       | yes      | Protocol version. Must be `1`.                                                                           |
| `type`           | string        | yes      | Invocation type. One of: `call`, `cast`, `stream`, `channel`, `batch`, `resource`, `log`, `announce`.    |
| `id`             | string (uuid) | yes      | Globally unique invocation ID. 16-byte UUID.                                                             |
| `target`         | string        | yes      | Fully-qualified function name: `namespace.function`. Omitted for `batch` (items have their own targets). |
| `args`           | array         | yes      | Positional arguments. Empty array `[]` if none.                                                          |
| `meta`           | object        | no       | Optional key-value metadata (trace IDs, request context).                                                |
| `capability`     | string        | no       | Capability token. Required if the function declares capabilities.                                        |
| `batch_items`    | array         | no       | For `batch` type: array of individual call envelopes.                                                    |
| `stream_control` | string        | no       | Backpressure/lifecycle signal for stream/channel.                                                        |
| `seq`            | integer       | no       | Sequence number for stream/channel frames.                                                               |

### Invocation Types

| Type       | Behavior                          | Response Expected            |
|------------|-----------------------------------|------------------------------|
| `call`     | Request/response                  | Yes - single response        |
| `cast`     | Fire-and-forget                   | No                           |
| `stream`   | Server-to-client ordered sequence | Yes - stream of items + end  |
| `channel`  | Bidirectional with backpressure   | Yes - per-direction messages |
| `batch`    | Multiple independent calls        | Yes - ordered results array  |
| `resource` | Opaque handle to external data    | Yes - ResourceHandle         |
| `log`      | Structured log record             | No (runtime sink)            |
| `announce` | Schema announcement (dev mode)    | Yes - announce_ack           |

## Response Envelope

```json
{
  "id": "<uuid>",
  "ok": true | false,
  "result": <value>,
  "error": { "code": "...", "message": "...", "details": { ... } },
  "seq": 0,
  "stream_control": "end" | "pause" | "resume" | "abort"
}
```

| Field            | Type    | Required    | Description                         |
|------------------|---------|-------------|-------------------------------------|
| `id`             | string  | yes         | ID from original invocation         |
| `ok`             | boolean | yes         | `true` if succeeded                 |
| `result`         | any     | if ok=true  | Return value                        |
| `error`          | object  | if ok=false | Error details                       |
| `seq`            | integer | no          | Stream/channel sequence number      |
| `stream_control` | string  | no          | Lifecycle signal for stream/channel |

## StreamControl

Used in both directions for stream and channel management:

| Value    | Direction         | Meaning                               |
|----------|-------------------|---------------------------------------|
| `end`    | Provider → Caller | No more items; stream half-closed     |
| `pause`  | Receiver → Sender | Buffer full; sender must pause        |
| `resume` | Receiver → Sender | Buffer ready; sender may continue     |
| `abort`  | Either            | Unrecoverable error; both sides close |

## Error Codes

| Code                   | When it happens                                        |
|------------------------|--------------------------------------------------------|
| `NamespaceNotFound`    | The requested namespace is not registered              |
| `FunctionNotFound`     | The requested function does not exist in its namespace |
| `InvalidArguments`     | Arguments failed type/shape validation                 |
| `IncompatibleVersion`  | Envelope protocol version is incompatible              |
| `MalformedEnvelope`    | A required field was missing from an envelope          |
| `NoProvider`           | No provider registered for the target namespace        |
| `ProviderUnavailable`  | Provider is temporarily unavailable                    |
| `BatchRoutingConflict` | Batch item resolved to a different namespace           |
| `CapabilityDenied`     | Caller lacks a required capability                     |
| `CapabilityInvalid`    | Capability token is invalid or expired                 |
| `ConnectionLost`       | Transport connection was lost                          |
| `MessageTooLarge`      | Message exceeded the size limit                        |
| `Timeout`              | Call exceeded its timeout                              |
| `BufferOverflow`       | Receive buffer overflowed                              |
| `ProviderError`        | Provider handler returned an error                     |
| `ProviderPanic`        | Provider panicked while handling invocation            |
| `StreamClosed`         | Stream was already closed                              |
| `ChannelClosed`        | Channel was closed by the remote side                  |
| `OutOfOrder`           | Out-of-order sequence number on ordered stream         |
| `Internal`             | Unspecified internal error                             |

## Announce Envelope

Providers send this to announce their schema in dev mode:

```json
{
  "version": 1,
  "type": "announce",
  "id": "<uuid>",
  "target": "$saikuro.announce",
  "args": [{ "namespace": "math", "functions": { ... }, "types": { ... } }]
}
```

The runtime acknowledges with:

```json
{
  "id": "<uuid>",
  "ok": true
}
```

## Log Envelope

```json
{
  "version": 1,
  "type": "log",
  "id": "<uuid>",
  "target": "$log",
  "args": [{ "ts": "2025-01-01T00:00:00Z", "level": "info", "name": "myapp", "msg": "started", "fields": { "version": "1.0" } }]
}
```

Log envelopes are never routed to a provider. The runtime extracts the record and passes it to the configured log sink. Fire-and-forget; no response.

## Message Ordering

| Primitive | Guarantee                                                         |
|-----------|-------------------------------------------------------------------|
| `call`    | Strict request/response. One response per call, correlated by ID. |
| `cast`    | No response. Best-effort delivery, ordered per connection.        |
| `stream`  | Ordered by `seq`. Frames arrive in send order.                    |
| `channel` | Ordered per-direction. Each direction independently ordered.      |
| `batch`   | Results returned in call order.                                   |

## Max Frame Size

Frames larger than 16 MiB are rejected to prevent memory exhaustion.

## Protocol Version

The `version` field is `1` for all current messages. Future versions will be additive: new optional fields may be added; existing fields will not be removed or renamed within a version.

If you send a version the runtime does not recognize, the envelope is rejected with a `SchemaError`.

## Implementing a New Adapter

1. Use any compliant MessagePack library.
2. Implement `InMemoryTransport` first for testing without a network.
3. Implement `makeTransport`/`make_transport` address parsing.
4. Implement the envelope types matching the spec above.
5. Implement the receive loop that dispatches responses by ID.

See the existing adapter implementations for reference:

- [TypeScript](https://github.com/Nisoku/Saikuro/tree/wasm-stuff/Build/adapters/typescript)
- [Python](https://github.com/Nisoku/Saikuro/tree/wasm-stuff/Build/adapters/python)
- [Rust](https://github.com/Nisoku/Saikuro/tree/wasm-stuff/Build/adapters/rust)
- [C#](https://github.com/Nisoku/Saikuro/tree/wasm-stuff/Build/adapters/csharp)
- [C](https://github.com/Nisoku/Saikuro/tree/wasm-stuff/Build/adapters/c)
- [C++](https://github.com/Nisoku/Saikuro/tree/wasm-stuff/Build/adapters/cpp)
