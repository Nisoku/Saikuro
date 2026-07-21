---
type: concept
title: "Language Adapters"
description: "Saikuro adapter APIs for TypeScript, Python, Rust, C#, C, and C++"
source: "https://nisoku.org/Saikuro/docs/adapters/"
path: /adapters/
updated: 2026-07-21
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-07-21T10:15:41.725Z"
---
---
title: "Language Adapters"
description: "Saikuro adapter APIs for TypeScript, Python, Rust, C#, C, and C++"
---

Each Saikuro adapter wraps the MessagePack protocol and exposes a native API for the host language. All adapters support the same six invocation primitives and transport address format.

## Available Adapters

::: grids
::: grid
::: card "TypeScript" icon:file-code
Node.js, browser, and Bun. Full support for all transports including WebSocket and WasmHost.

::: button "TypeScript API" ./typescript/ icon:code
::: button "Examples" ./typescript/examples.md icon:terminal
:::
:::
::: grid
::: card "Python" icon:code
Python 3.11+ with asyncio. Unix socket, TCP, WebSocket, InMemory, and WasmHost transports.

::: button "Python API" ./python/ icon:code
::: button "Examples" ./python/examples.md icon:terminal
:::
:::
::: grid
::: card "Rust" icon:box
Full featured with Provider, Client, typed schema, and storage backend access.

::: button "Rust API" ./rust/ icon:code
::: button "Examples" ./rust/examples.md icon:terminal
:::
:::
::: grid
::: card "C#" icon:hash
.NET 8+ with Blazor WASM support. BroadcastChannel transport for browser hosting.

::: button "C# API" ./csharp/ icon:code
::: button "Examples" ./csharp/examples.md icon:terminal
:::
:::
::: grid
::: card "C" icon:terminal
FFI-friendly single header binding. MessagePack buffer interface.

::: button "C API" ./c/ icon:code
::: button "Examples" ./c/examples.md icon:terminal
:::
:::
::: grid
::: card "C++" icon:cpu
RAII wrapper over the C API. Header-only with template type mapping.

::: button "C++ API" ./cpp/ icon:code
::: button "Examples" ./cpp/examples.md icon:terminal
:::
:::
:::

## Capability Matrix

| Feature     |         TypeScript          |           Python            |            Rust             |             C#              |              C              |             C++             |
|-------------|:---------------------------:|:---------------------------:|:---------------------------:|:---------------------------:|:---------------------------:|:---------------------------:|
| Call        | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e |
| Cast        | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e |
| Stream      | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e |
| Channel     | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e |
| Batch       | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e |
| Resource    | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e |
| Log         | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e |
| InMemory    | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e |
| Unix Socket | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e |
| TCP         | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e |
| WebSocket   | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e |
| WasmHost    | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e |
| Codegen     | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e | ::: tag "Yes" color:#22c55e |

## Next Steps

::: grids
::: grid
::: button "Invocation Primitives" ../guide/invocations.md icon:zap
:::
::: grid
::: button "Transports" ../guide/transports.md icon:radio
:::
:::
