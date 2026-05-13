# Parity Measurement Table

## Legend

| Value   | Meaning                                         |
|---------|-------------------------------------------------|
| yes     | implemented and documented in adapter API       |
| partial | available indirectly or with reduced ergonomics |
| no      | currently missing                               |

## Transport Parity

| Capability                            | Rust           | TypeScript           | Python | C#         | C              | C++      |
|---------------------------------------|----------------|----------------------|--------|------------|----------------|----------|
| **Address Schemes**                   |                |                      |        |            |                |          |
| `tcp://host:port`                     | yes            | yes (Node)           | yes    | yes        | yes            | yes      |
| `unix:///path`                        | yes            | yes (Node)           | yes    | yes        | yes            | yes      |
| `ws://host/path`                      | yes            | yes                  | yes    | yes        | yes            | yes      |
| `wss://host/path`                     | yes            | yes                  | yes    | yes        | yes            | yes      |
| `wasm-host://channel`                 | yes            | yes                  | no     | no         | yes            | yes      |
| `wasm-host` (default)                 | yes            | yes                  | no     | no         | yes            | yes      |
| `memory://` (testing)                 | doc only       | no                   | yes    | no         | no             | no       |
| **Transport Implementations**         |                |                      |        |            |                |          |
| InMemoryTransport                     | yes            | yes                  | yes    | yes        | via Rust       | via Rust |
| TCP Transport                         | yes            | yes (Node)           | yes    | yes        | via Rust       | via Rust |
| Unix Socket Transport                 | yes            | yes (Node)           | yes    | yes        | via Rust       | via Rust |
| WebSocket Transport                   | yes            | yes                  | yes    | yes        | via Rust       | via Rust |
| BroadcastChannel Transport            | yes            | yes                  | no     | no         | via Rust       | via Rust |
| **Features / Configuration**          |                |                      |        |            |                |          |
| 4-byte BE length prefix (TCP/Unix)    | yes            | yes                  | yes    | yes        | yes            | yes      |
| 16 MiB max frame size                 | yes            | yes                  | yes    | yes        | yes            | yes      |
| raw MessagePack (WS/BroadcastChannel) | yes            | yes                  | yes    | yes        | yes            | yes      |
| Conditional compilation               | Cargo features | bundler tree-shaking | no     | `#if WASM` | Cargo features | via C    |

## WASM Parity

| Capability                  | Rust (wasm32)              | TypeScript (WASM Context)     | Python (Pyodide)             | C# (Blazor WASM)    | C/C++ (wasm32)       |
|-----------------------------|----------------------------|-------------------------------|------------------------------|---------------------|----------------------|
| **WASM Compilation**        |                            |                               |                              |                     |                      |
| WASM target supported       | yes                        | N/A (JS)                      | partial (Pyodide only)       | yes                 | yes                  |
| Feature flag for WASM       | `--features wasm`          | N/A                           | none                         | `#if WASM` constant | `--features wasm`    |
| **Runtime Transports**      |                            |                               |                              |                     |                      |
| InMemoryTransport           | yes                        | yes                           | partial                      | yes                 | yes                  |
| WebSocket Transport         | yes (needs `ws` feature)   | yes                           | no                           | yes                 | yes (via Rust)       |
| BroadcastChannel/WasmHost   | yes (needs `wasm` feature) | yes                           | no                           | no                  | yes (via Rust)       |
| TCP Transport               | no                         | no                            | no                           | no                  | no                   |
| Unix Socket Transport       | no                         | no                            | no                           | no                  | no                   |
| **Address Factory on WASM** |                            |                               |                              |                     |                      |
| `ws://` / `wss://`          | yes                        | yes                           | no                           | yes                 | yes                  |
| `wasm-host://`              | yes                        | yes                           | no                           | no                  | yes                  |
| `tcp://`                    | no                         | no                            | no                           | no                  | no                   |
| `unix://`                   | no                         | no                            | no                           | no                  | no                   |
| **Runtime Environment**     |                            |                               |                              |                     |                      |
| WASM executor               | wasm-bindgen-futures       | N/A (JS event loop)           | Pyodide event loop           | Blazor JS interop   | wasm-bindgen-futures |
| Uses tokio?                 | no (wasm-runtime feature)  | N/A                           | no                           | no                  | no (wasm-runtime)    |
| Uses `asyncio`?             | no                         | N/A                           | no (Pyodide asyncio differs) | N/A                 | no                   |
| **Interop Capability**      |                            |                               |                              |                     |                      |
| Rust WASM ↔ JS              | yes (wasm-bindgen)         | N/A                           | yes (Pyodide)                | yes (IJSRuntime)    | yes (via Rust)       |
| BroadcastChannel JS API     | via `web-sys` crate        | direct via `BroadcastChannel` | via `js` module              | via JS interop      | via Rust `web-sys`   |

## Capability Parity

| Capability                  | Rust | TypeScript | Python | C#  | C   | C++ |
|-----------------------------|------|------------|--------|-----|-----|-----|
| call                        | yes  | yes        | yes    | yes | yes | yes |
| cast                        | yes  | yes        | yes    | yes | yes | yes |
| batch                       | yes  | yes        | yes    | yes | yes | yes |
| stream                      | yes  | yes        | yes    | yes | yes | yes |
| channel                     | yes  | yes        | yes    | yes | yes | yes |
| resource invocation helpers | yes  | yes        | yes    | yes | yes | yes |
| log forwarding helper       | yes  | yes        | yes    | yes | yes | yes |
| provider registration       | yes  | yes        | yes    | yes | yes | yes |

## Tooling Parity

| Capability           | Rust | TypeScript | Python | C#  | C   | C++ |
|----------------------|------|------------|--------|-----|-----|-----|
| schema extractor CLI | yes  | yes        | yes    | yes | yes | yes |
| typed codegen output | yes  | yes        | yes    | yes | yes | yes |

## Test Coverage

| Test                        | Rust | TypeScript | Python | C#  | C   | C++ |
|-----------------------------|------|------------|--------|-----|-----|-----|
| call                        | yes  | yes        | yes    | yes | yes | yes |
| cast                        | yes  | yes        | yes    | yes | yes | yes |
| batch                       | yes  | yes        | yes    | yes | yes | yes |
| stream                      | yes  | yes        | yes    | yes | yes | yes |
| channel                     | yes  | yes        | yes    | yes | yes | yes |
| resource invocation helpers | yes  | yes        | yes    | yes | yes | yes |
| log forwarding helper       | yes  | yes        | yes    | yes | yes | yes |
| provider registration       | yes  | yes        | yes    | yes | yes | yes |
| schema extractor CLI tests  | yes  | yes        | yes    | yes | yes | yes |
| typed codegen output        | yes  | yes        | yes    | yes | yes | yes |
| envelope roundtrip          | yes  | yes        | yes    | yes | yes | yes |
| transport behavior          | yes  | yes        | yes    | yes | yes | yes |
| timeout and cancellation    | yes  | yes        | yes    | yes | yes | yes |
| error mapping propagation   | yes  | yes        | yes    | yes | yes | yes |
| announce handshake          | yes  | yes        | yes    | yes | yes | yes |
| core runtime integration    | yes  | yes        | yes    | yes | yes | yes |
