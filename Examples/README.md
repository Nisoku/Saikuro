# Examples

Runnable examples showing Saikuro providers and clients in Rust, TypeScript, Python,
and browser wasm.

Every example registers `math.add`, `math.subtract`, `math.multiply`, and `math.divide`,
then exercises call, cast, batch, and error-handling paths. The Rust examples share one
`math-core` crate for the provider, the schema, and the client demo, so the only per-target
code is the transport wiring.

## Structure

```text
Examples/
  rust/
    math-core/          shared provider, schema, and client demo
    math/               native: provider + client in one process
    math-wasi-preview1/ wasm32-wasip1
    math-wasi-preview2/ wasm32-wasip2
    math-wasm/          wasm32-unknown-unknown (browser)
  typescript/math/      provider + client over memory or TCP
  python/math/          provider + client over memory or TCP
  browser/              index.html and the wasm-pack output
```

## Transport topology

The native, TypeScript, and Python examples host a provider and dial it in the
same process, so no external runtime is needed:

- **in-memory** - paired channels, zero-copy, zero-latency.
- **TCP** - the provider listens on loopback, the client dials it. `--serve-only`
  is not available outside Rust; the other examples dial a provider they start
  themselves.

`--transport memory` is the default everywhere. `--transport tcp` accepts
`--addr HOST:PORT`; the defaults are `127.0.0.1:9000` (Rust, TypeScript) and
`127.0.0.1:7700` (Python).

## Running

### Rust, native

```bash
cd Examples/rust
cargo run -p math                                    # in-memory
cargo run -p math -- --transport tcp                 # loopback TCP
cargo run -p math -- --transport tcp --addr 127.0.0.1:9000
```

`--serve-only` listens without a local client, for driving from another process
or another language:

### Rust, WASI preview 1 and 2

```bash
cd Examples/rust
cargo run -p math-wasi-preview1 --target wasm32-wasip1
cargo run -p math-wasi-preview2 --target wasm32-wasip2
```

### Browser wasm

```bash
cd Examples/rust/math-wasm
wasm-pack build --target web --out-dir ../../browser/pkg

# Serve the directory. Cross-origin isolation headers are required:
#   Cross-Origin-Opener-Policy: same-origin
#   Cross-Origin-Embedder-Policy: require-corp
npx serve Examples/browser
```

`index.html` runs the in-memory demo on load. The in-realm section takes a
channel name: press **Provide** in one tab and **Dial** in another to run the
provider and client over a `BroadcastChannel`. Tab order does not
matter, the client half re-announces until the accept lands.

### TypeScript

Requires Node 24. Build the adapter once, then run the example:

```bash
cd Build/adapters/typescript && npm install && npm run build

cd Examples/typescript/math
npm install && npm run build
npm start                                # in-memory
npm start -- --transport tcp             # loopback TCP
npm start -- --transport tcp --addr 127.0.0.1:9000
```

### Python

Requires Python 3.11+:

```bash
pip install -e Build/adapters/python

cd Examples/python/math
python main.py                            # in-memory
python main.py --transport tcp            # loopback TCP
python main.py --transport tcp --addr 127.0.0.1:7700
```
