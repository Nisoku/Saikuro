---
type: concept
title: "Python API Reference"
description: "Full Python adapter API reference"
source: "https://nisoku.org/Saikuro/adapters/python/api-reference/"
path: /adapters/python/api-reference/
updated: 2026-07-09
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-07-09T20:42:46.330Z"
---
---
title: "Python API Reference"
description: "Full Python adapter API reference"
---

See the [Python adapter overview](./) for the complete API surface.

## SaikuroClient

| Method           | Signature                                                 | Description                       |
|------------------|-----------------------------------------------------------|-----------------------------------|
| `connect`        | `classmethod (address: str) -> SaikuroClient`             | Connect to runtime at address     |
| `from_transport` | `classmethod (transport) -> SaikuroClient`                | From transport without connecting |
| `open_on`        | `classmethod (transport) -> SaikuroClient`                | Connect on existing transport     |
| `close`          | `() -> None`                                              | Gracefully close                  |
| `call`           | `(target, args, capability?, timeout?) -> Any`            | Request/response                  |
| `cast`           | `(target, args, capability?) -> None`                     | Fire-and-forget                   |
| `stream`         | `(target, args, capability?) -> SaikuroStream`            | Open a stream                     |
| `channel`        | `(target, args, capability?) -> SaikuroChannel`           | Open a channel                    |
| `batch`          | `(calls, timeout?) -> list`                               | Batch multiple calls              |
| `resource`       | `(target, args, capability?, timeout?) -> ResourceHandle` | Resource handle                   |
| `log`            | `(level, name, msg, fields?) -> None`                     | Structured log                    |

## SaikuroProvider

| Method              | Signature                                     | Description                   |
|---------------------|-----------------------------------------------|-------------------------------|
| `__init__`          | `(namespace: str)`                            | Create provider for namespace |
| `register`          | `(name, *, capabilities?, doc?) -> Decorator` | Decorator registration        |
| `register_function` | `(name, fn, *, capabilities?, doc?) -> None`  | Imperative registration       |
| `serve`             | `(address: str) -> None`                      | Serve on address              |
| `serve_on`          | `(transport) -> None`                         | Serve on existing transport   |
| `schema_dict`       | `() -> dict`                                  | Build schema dict             |
