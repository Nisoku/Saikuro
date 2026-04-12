---
title: "Adapters"
description: "Language-specific adapter guides"
---

Adapters are the language boundary for Saikuro. They expose a familiar API in each language while speaking the same wire protocol.

## Choose a language

- [C](./c/)
- [C++](./cpp/)
- [C#](./csharp/)
- [Python](./python/)
- [Rust](./rust/)
- [TypeScript](./typescript/)

## Adapter responsibilities

Each adapter handles:

- Envelope encoding/decoding and transport I/O
- Provider and client ergonomics for its language
- Stream and channel state at the caller API boundary
- Consistent error mapping (`code`, `message`, optional `details`)

The runtime handles routing, schema enforcement, capability checks, and dispatch policy.

## Shared model

All adapters expose the same primitives:

- `call`: request/response invocation
- `cast`: fire-and-forget invocation
- `stream`: server-to-client async sequence
- `channel`: bidirectional async message flow
- `batch`: multiple invocations in one envelope
- `resource`: handle-based stateful operations

## Capability parity

| Adapter | Call | Cast | Stream | Channel | Batch | Resource | Schema extraction |
| ------- | ---- | ---- | ------ | ------- | ----- | -------- | ----------------- |
| TypeScript | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Python | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Rust | Yes | Yes | Yes | Yes | Yes | Yes | Manual/runtime metadata |
| C# | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| C | Yes | Yes | Yes | Yes | Yes | Yes | Via codegen/tooling |
| C++ | Yes | Yes | Yes | Yes | Yes | Yes | `saikuro-cpp-schema` |

## Examples by adapter

- [TypeScript examples](./typescript/examples)
- [Python examples](./python/examples)
- [Rust examples](./rust/examples)
- [C# examples](./csharp/examples)
- [C examples](./c/examples)
- [C++ examples](./cpp/examples)

## API references by adapter

- [TypeScript API reference](./typescript/api-reference)
- [Python API reference](./python/api-reference)
- [Rust API reference](./rust/api-reference)
- [C# API reference](./csharp/api-reference)
- [C API reference](./c/api-reference)
- [C++ API reference](./cpp/api-reference)

## Next Steps

- [Invocation Primitives](../guide/invocations): Semantics of each primitive
- [Transports](../guide/transports): Transport behavior and tradeoffs
- [Protocol Reference](../api/): Envelope and error format