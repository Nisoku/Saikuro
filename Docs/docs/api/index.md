---
title: "Core Protocol & Runtime Reference"
description: "Shared wire format and runtime behavior for all adapters"
---

Everything Saikuro sends over the wire is a MessagePack-encoded envelope. This page documents the full shape of every message type.

This is the shared core reference only (protocol/runtime behavior that applies across all adapters). Adapter-specific client/provider APIs now live under the Adapters section.

## Encoding

All envelopes use [MessagePack](https://msgpack.org/). MessagePack is a compact binary format, similar to JSON but smaller and faster to parse. You don't need to handle encoding yourself; the adapters do it for you. This reference is for anyone implementing a new adapter or debugging raw traffic.

## Invocation Envelope

Sent by the caller to initiate any invocation.

```json
{
  "version": 1,
  "type": "call" | "cast" | "stream" | "channel" | "batch",
  "id": "<uuid>",
  "target": "namespace.function",
  "args": [...],
  "meta": { ... },
  "cap": "<capability-token>"
}
```

| Field | Type | Required | Description |
| ----- | ---- | -------- | ----------- |
| `version` | integer | yes | Protocol version. Must be `1` in v1. |
| `type` | string | yes | Invocation primitive. One of: `call`, `cast`, `stream`, `channel`, `batch`. |
| `id` | string | yes | Globally unique ID for this invocation. UUID v4 recommended. |
| `target` | string | yes | Fully-qualified function name: `namespace.function`. Not used for `batch` (see below). |
| `args` | array | yes | Positional arguments. Empty array `[]` if the function takes no arguments. |
| `meta` | object | no | Optional key-value metadata (trace IDs, request context, etc). |
| `cap` | string | no | Capability token. Required if the function declares capabilities. |

### Example: Call

```json
{
  "version": 1,
  "type": "call",
  "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "target": "math.add",
  "args": [1, 2]
}
```

### Example: Cast

```json
{
  "version": 1,
  "type": "cast",
  "id": "b2c3d4e5-f6a7-8901-bcde-f12345678901",
  "target": "log.write",
  "args": [{ "level": "info", "message": "started" }]
}
```

No response is sent for `cast`.

## Response Envelope

Sent by the runtime back to the caller for `call` invocations.

```json
{
  "id": "<uuid>",
  "ok": true | false,
  "result": <value>,
  "error": <error-object>
}
```

| Field | Type | Required | Description |
| ----- | ---- | -------- | ----------- |
| `id` | string | yes | The ID from the original invocation envelope. Used to correlate responses. |
| `ok` | boolean | yes | `true` if the call succeeded, `false` if it failed. |
| `result` | any | if ok=true | The return value. May be `null` for `void` functions. |
| `error` | object | if ok=false | Error details. See Error Envelope below. |

### Example: Success

```json
{
  "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "ok": true,
  "result": 3
}
```

### Example: Failure

```json
{
  "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "ok": false,
  "error": {
    "code": "InvalidArgs",
    "message": "expected i32, got string",
    "details": { "arg": 0, "expected": "i32", "got": "string" }
  }
}
```

## Stream Frames

For `stream` invocations, the provider sends a sequence of frames.

### Stream Frame (data)

```json
{
  "id": "<uuid>",
  "seq": <integer>,
  "data": <value>
}
```

| Field | Type | Description |
| ----- | ---- | ----------- |
| `id` | string | The ID from the original stream invocation. |
| `seq` | integer | Sequence number, starting at 0. Monotonically increasing. |
| `data` | any | The stream item value. |

### Stream Frame (end)

```json
{
  "id": "<uuid>",
  "seq": <integer>,
  "end": true
}
```

Signals that the stream is finished. No more frames will follow for this ID.

### Stream Frame (error)

```json
{
  "id": "<uuid>",
  "seq": <integer>,
  "error": <error-object>
}
```

Signals that the stream ended with an error. No more frames will follow for this ID.

## Channel Frames

Channels are bidirectional. Both sides can send frames.

### Channel Frame (data)

Same shape as stream frames, but flows in both directions:

```json
{
  "id": "<uuid>",
  "seq": <integer>,
  "data": <value>
}
```

### Channel Close

Either side can close the channel:

```json
{
  "id": "<uuid>",
  "close": true
}
```

When either side sends a close frame, the other side sees EOF on its incoming iterator. Both sides must send a close frame for the channel to be fully terminated.

### Backpressure

The runtime tracks per-channel buffer capacity. If a sender is faster than the receiver, the runtime will signal the sender to pause by not acknowledging frames until capacity is available. The adapters handle this transparently, so your `send()` call will yield until there's room.

## Batch Envelope

A batch groups multiple calls into one envelope.

```json
{
  "version": 1,
  "type": "batch",
  "id": "<uuid>",
  "batch_items": [
    {
      "version": 1,
      "type": "call",
      "id": "<uuid>",
      "target": "math.add",
      "args": [1, 2]
    },
    {
      "version": 1,
      "type": "call",
      "id": "<uuid>",
      "target": "math.multiply",
      "args": [3, 4]
    }
  ],
  "cap": "<capability-token>"
}
```

Each call in the batch has its own `id` and `target`. The top-level `id` is for the batch itself.

### Batch Response

```json
{
  "id": "<batch-uuid>",
  "ok": true,
  "results": [
    { "id": "<call-uuid>", "ok": true, "result": 3 },
    { "id": "<call-uuid>", "ok": true, "result": 12 }
  ]
}
```

Results are returned in the same order as the calls. If any individual call fails, its result entry has `ok: false` with an error. The batch itself reports `ok: false` if any call fails.

## Error Envelope

Used inside response, stream, and batch envelopes.

```json
{
  "code": "NotFound" | "InvalidArgs" | "CapabilityDenied" | "ProviderError" | "RoutingError" | "TransportError" | "SchemaError" | "Timeout",
  "message": "human-readable description",
  "details": { ... }
}
```

### Error Codes

| Code | When it happens |
| ---- | --------------- |
| `NotFound` | The target namespace or function does not exist |
| `InvalidArgs` | Arguments don't match the declared schema |
| `CapabilityDenied` | Caller doesn't have a required capability |
| `ProviderError` | The provider threw an exception during execution |
| `RoutingError` | The runtime couldn't route to a provider (no provider registered for the namespace) |
| `TransportError` | The transport connection failed or was lost |
| `SchemaError` | Schema validation failed (invalid schema structure) |
| `Timeout` | The call exceeded its timeout |

## Announce Envelope

Providers send this to announce their schema in dev mode.

```json
{
  "type": "announce",
  "namespace": "math",
  "schema": {
    "version": 1,
    "namespaces": { ... },
    "types": { ... }
  }
}
```

The runtime acknowledges with:

```json
{
  "type": "announce_ack",
  "namespace": "math",
  "ok": true
}
```

If the announced schema conflicts with one already registered, `ok` is `false` with a `SchemaError`.

## Message Ordering Guarantees

| Primitive | Ordering guarantee |
| --------- | ------------------ |
| `call` | Strict request/response. One response per call, correlated by ID. |
| `cast` | No response. Delivery is best-effort, ordered per connection. |
| `stream` | Ordered by `seq`. Frames arrive in the order they were sent. |
| `channel` | Ordered per direction. Caller-to-provider and provider-to-caller are each independently ordered. |
| `batch` | Results returned in call order regardless of internal execution order. |

## Protocol Version

The `version` field in invocation envelopes is `1` for all current messages. Future versions will be additive: new optional fields may be added, existing fields will not be removed or renamed within a version.

If you send a version the runtime doesn't recognize, the runtime rejects the envelope with a `SchemaError`.

## Implementing a New Adapter

If you're writing a Saikuro adapter for a language not listed here:

1. Use a MessagePack library for your language. Any compliant implementation works.
2. Implement `InMemoryTransport` first; it lets you test the full protocol without a network.
3. Follow the ordering rules above exactly. Getting sequence numbers wrong on streams causes subtle bugs.
4. The announce handshake must register the listener before sending the announce envelope, or the ack can be missed on fast transports.
5. For channel close, complete the send-side writer (not the receive-side) to signal EOF to the peer.

The TypeScript, Python, and C# adapters are the reference implementations. When in doubt, read those.
