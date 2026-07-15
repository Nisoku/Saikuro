---
type: concept
title: "TypeScript API Reference"
description: "Full TypeScript adapter API reference"
source: "https://nisoku.org/Saikuro/adapters/typescript/api-reference/"
path: /adapters/typescript/api-reference/
updated: 2026-07-15
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-07-15T17:26:24.470Z"
---
---
title: "TypeScript API Reference"
description: "Full TypeScript adapter API reference"
---

See the [TypeScript adapter overview](./) for the complete API surface.

## SaikuroClient

| Method          | Signature                                                                        | Description                       |
|-----------------|----------------------------------------------------------------------------------|-----------------------------------|
| `connect`       | `static (address: string, options?: ClientOptions): Promise<SaikuroClient>`      | Connect to runtime at address     |
| `openOn`        | `static (transport: Transport, options?: ClientOptions): Promise<SaikuroClient>` | Connect on existing transport     |
| `fromTransport` | `static (transport: Transport, options?: ClientOptions): SaikuroClient`          | From transport without connecting |
| `open`          | `(): Promise<void>`                                                              | Connect the transport             |
| `close`         | `(): Promise<void>`                                                              | Gracefully close                  |
| `call`          | `(target, args, options?): Promise<unknown>`                                     | Request/response                  |
| `cast`          | `(target, args, options?): Promise<void>`                                        | Fire-and-forget                   |
| `stream`        | `<T>(target, args, options?): Promise<SaikuroStream<T>>`                         | Open a stream                     |
| `channel`       | `<TIn, TOut>(target, args, options?): Promise<SaikuroChannel<TIn, TOut>>`        | Open a channel                    |
| `batch`         | `(calls, options?): Promise<unknown[]>`                                          | Batch multiple calls              |
| `resource`      | `(target, args, options?): Promise<ResourceHandle>`                              | Resource handle                   |
| `log`           | `(level, name, msg, fields?): Promise<void>`                                     | Structured log                    |

## SaikuroProvider

| Method         | Signature                              | Description                   |
|----------------|----------------------------------------|-------------------------------|
| `constructor`  | `(namespace: string)`                  | Create provider for namespace |
| `register`     | `(name, handler, options?): this`      | Register a function           |
| `decorator`    | `(name, options?): Decorator`          | Decorator registration        |
| `schemaObject` | `(): SaikuroSchema`                    | Build schema object           |
| `dispatch`     | `(envelope, transport): Promise<void>` | Handle an invocation          |
| `serve`        | `(address: string): Promise<void>`     | Serve on address              |
| `serveOn`      | `(transport, options?): Promise<void>` | Serve on existing transport   |
