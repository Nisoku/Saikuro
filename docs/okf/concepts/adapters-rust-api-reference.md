---
type: concept
title: "Rust API Reference"
description: "Full Rust adapter API reference"
source: "https://nisoku.org/Saikuro/adapters/rust/api-reference/"
path: /adapters/rust/api-reference/
updated: 2026-06-27
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-06-27T13:18:33.177Z"
---
---
title: "Rust API Reference"
description: "Full Rust adapter API reference"
---

See the [Rust adapter overview](./) for the complete API surface.

## Client

| Method    | Signature                                                     | Description                   |
|-----------|---------------------------------------------------------------|-------------------------------|
| `connect` | `(address: &str) -> Result<Client>`                           | Connect to runtime at address |
| `open_on` | `(transport: InMemoryTransport) -> Result<Client>`            | Connect on InMemory transport |
| `call`    | `<T: DeserializeOwned>(&mut self, target, args) -> Result<T>` | Request/response              |
| `cast`    | `(&mut self, target, args) -> Result<()>`                     | Fire-and-forget               |
| `stream`  | `(&mut self, target, args) -> Result<SaikuroStream>`          | Open a stream                 |
| `channel` | `(&mut self, target, args) -> Result<SaikuroChannel>`         | Open a channel                |
| `batch`   | `(&mut self, calls) -> Result<Vec<Value>>`                    | Batch multiple calls          |
| `close`   | `(&mut self) -> Result<()>`                                   | Gracefully close              |

## Provider

| Method     | Signature                                                 | Description                   |
|------------|-----------------------------------------------------------|-------------------------------|
| `new`      | `(namespace: &str) -> Provider`                           | Create provider for namespace |
| `register` | `(&mut self, name, handler) -> RegisterHandle`            | Register a handler            |
| `serve`    | `(&mut self, address: &str) -> Result<()>`                | Serve on address              |
| `serve_on` | `(&mut self, transport: InMemoryTransport) -> Result<()>` | Serve on InMemory             |
