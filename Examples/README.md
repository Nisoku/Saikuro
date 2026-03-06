# Examples

Runnable examples showing Saikuro providers and clients in Rust, TypeScript, and Python.

Each example is self-contained: a provider and client are wired together in a
single process using an in-memory transport. No live runtime is needed.

## Structure

```
Examples/
  rust/
    math/   combined provider + client (Rust)
  typescript/
    math/   combined provider + client (TypeScript)
  python/
    math/   combined provider + client (Python)
```

Each example registers `math.add`, `math.subtract`, `math.multiply`, and
`math.divide`, then exercises call, cast, batch, and error-handling paths.

## Running

### Rust

```bash
cd Examples/rust
cargo run -p math
```

### TypeScript

Requires Node 22. Build the adapter once, then run the example:

```bash
cd Build/adapters/typescript && npm install && npm run build

# Then
cd Examples/typescript/math
npm install && npm run build
npm run start
```

### Python

Requires Python 3.11+:

```bash
pip install -e Build/adapters/python

cd Examples/python/math
python main.py
```
