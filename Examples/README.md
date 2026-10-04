# Examples

Runnable Saikuro examples: providers and clients in Rust, TypeScript, Python, and
browser wasm.

Every example registers `math.add`, `math.subtract`, `math.multiply`, and
`math.divide`, then exercises call, cast, batch, and error paths. The Rust
examples share the `math-core` crate (provider, schema, client demo), so each
target only wires up its transport.

## Layout

```text
Examples/
  rust/
    math-core/          shared provider, schema, and client demo
    math/               native: provider + client in one process
    math-wasi-preview1/ wasm32-wasip1 (memory only)
    math-wasi-preview2/ wasm32-wasip2
    math-wasm/          wasm32-unknown-unknown (browser)
  typescript/math/      provider + client over memory, TCP, Unix, or WebSocket
  python/math/          provider + client over memory, TCP, Unix, or WebSocket
  browser/              index.html and the wasm-pack output
```

## Transports

Native, TypeScript, and Python host a provider and dial it in the same process,
so nothing external is needed. `--transport memory` is the default everywhere.

- **memory** - paired channels, no copy, no latency.
- **TCP** - provider listens on loopback, client dials it.
- **Unix** - provider listens on a filesystem path, client dials it (native Unix only).
- **WebSocket** - provider listens on loopback, client dials it.

The Rust examples take `--serve-only` (listen, no local client) and
`--client-only` (dial, don't listen), which is how you drive one from another
process or language. TypeScript and Python always host and dial in-process.

| target              | memory | tcp | unix |     ws      |
| ------------------- | :----: | :-: | :--: | :---------: |
| Rust native         |  yes   | yes | yes  |     yes     |
| Rust WASI preview 2 |  yes   | yes |  no  |     yes     |
| Rust WASI preview 1 |  yes   | no  |  no  |     no      |
| Browser wasm        |  yes   | no  |  no  | client-only |
| TypeScript          |  yes   | yes | yes  |     yes     |
| Python              |  yes   | yes | yes  |     yes     |

A WebSocket client is client-only in the browser because the platform has no
listening socket, so a browser must dial a provider running elsewhere. The WASI
backend is a full server and client; a WASI provider listens on loopback like the
TCP one:

```bash
cargo run -p math -- --transport ws --addr 127.0.0.1:9000 --serve-only
```

`--transport tcp` and `--transport ws` take `--addr HOST:PORT`. The Rust
defaults use port `0`, so the OS picks an ephemeral port and the example prints
the bound address.

## Running

### Rust, native

```bash
cd Examples/rust
cargo run -p math                                    # memory
cargo run -p math -- --transport tcp                 # loopback TCP
cargo run -p math -- --transport tcp --addr 127.0.0.1:9000
cargo run -p math -- --transport unix                # loopback Unix socket
cargo run -p math -- --transport ws                  # loopback WebSocket
```

`--serve-only` and `--client-only` split it across processes:

```bash
cargo run -p math -- --transport tcp --addr 127.0.0.1:9000 --serve-only
cargo run -p math -- --transport tcp --addr 127.0.0.1:9000 --client-only
```

### Rust, WASI preview 2

Supports `memory`, `tcp`, and `ws`, each as a provider or a client. Wasmtime
blocks sockets unless you pass `-S inherit-network=y`. The repo's
`.cargo/config.toml` runs the target through
`wasmtime --wasi common -S inherit-network=y`, which is enough for all three:

```bash
cd Examples/rust
cargo run -p math-wasi-preview2 --target wasm32-wasip2
cargo run -p math-wasi-preview2 --target wasm32-wasip2 -- --transport ws
```

To split the provider and the client across processes, run the built module
through `wasmtime` directly:

```bash
cd Examples/rust
wasmtime run -S inherit-network=y \
    target/wasm32-wasip2/debug/math-wasi-preview2.wasm \
    --transport ws --addr 127.0.0.1:9000 --serve-only
wasmtime run -S inherit-network=y \
    target/wasm32-wasip2/debug/math-wasi-preview2.wasm \
    --transport ws --addr ws://127.0.0.1:9000 --client-only
```

### Rust, WASI preview 1

Memory only, and not runnable on current runtimes. See
[WASI limitations](#wasi-limitations).

```bash
cd Examples/rust
cargo run -p math-wasi-preview1 --target wasm32-wasip1
```

### Browser wasm

```bash
cd Examples/rust/math-wasm
wasm-pack build --target web --out-dir ../../browser/pkg

# Serve from here. The page needs cross-origin isolation:
#   Cross-Origin-Opener-Policy: same-origin
#   Cross-Origin-Embedder-Policy: require-corp
npx serve ../../browser
```

`index.html` runs the memory demo on load. The in-realm section takes a channel
name: press **Provide** in one tab and **Dial** in another to run over a
`BroadcastChannel` (the browser's same-origin multi-tab transport).

### TypeScript

Node 24. Build the adapter once, then the example:

```bash
cd Build/adapters/typescript && npm install && npm run build

cd Examples/typescript/math
npm install && npm run build
npm start                                # memory
npm start -- --transport tcp             # loopback TCP
npm start -- --transport unix
npm start -- --transport ws
```

### Python

Python 3.11+:

```bash
pip install -e Build/adapters/python

cd Examples/python/math
python main.py                            # memory
python main.py --transport tcp            # loopback TCP
python main.py --transport unix
python main.py --transport ws
```

## Limitations

### WASI

`saikuro-transport` has a WASI TCP backend under
`Build/crates/saikuro-transport/wasi/`, with preview1 and preview2 backends
behind the `wasi-tcp` feature. Preview 2 uses the `wasi` crate's sockets API;
preview 1 calls the legacy `wasi_snapshot_preview1::sock_*` ABI directly.

Preview 2 is fully async: accept, connect, read, and write poll the socket's
`Pollable` and return `Pending` instead of blocking, so a provider and a client
interleave correctly in one single-threaded guest. The WebSocket transport is
async in both directions, driving `embedded-websocket` frames over a pollable
TCP stream.

Preview 1 has no non-blocking receive and no usable pollable, so `sock_recv` and
`sock_send` park the host thread so preview 1 TCP can't run today and would need a pollable socket ABI preview 1 doesn't define.

### Browser

Browser WebSocket lives in `Build/crates/saikuro-transport/wasm/websocket.rs`: a
client transport over `web_sys::WebSocket`, gated on `feature = "ws"` plus
`feature = "std"`, exposed by `index.html`. A browser can only be a client, so
that mode needs an external provider or relay, not a second tab.
