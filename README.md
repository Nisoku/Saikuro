# Saikuro

A language-agnostic, easy-to-use IPC library for cross-language integration.

Saikuro lets you expose functions written in one language and call them
transparently from any other supported language, with a shared typed schema,
capability enforcement, and pluggable transports (TCP, Unix socket, WebSocket,
or in-memory).

---

## Supported languages

(Saikuro currently is not added to package managers yet. It will be added soon, once I finalize a few things!)
| Language | Package name | Status |
|------------|--------------|--------|
| Rust | `saikuro` (crates.io) | ✅ |
| TypeScript | `saikuro` (npm) | ✅ |
| Python | `saikuro` (PyPI) | ✅ |
| C# | `Saikuro` (NuGet) | ✅ |

---

## Quick start

### Rust provider

```rust
use saikuro::{Provider, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let mut provider = Provider::new("math");

    provider.register("add", |args: Vec<serde_json::Value>| async move {
        let a = args[0].as_i64().unwrap_or(0);
        let b = args[1].as_i64().unwrap_or(0);
        Ok(serde_json::json!(a + b))
    });

    provider.serve("tcp://127.0.0.1:7700").await
}
```

### TypeScript client

```typescript
import { Client } from "@nisoku/saikuro";

const client = await Client.connect("tcp://127.0.0.1:7700");
const result = await client.call("math.add", [1, 2]);
console.log(result); // 3
```

### Python client

```python
from saikuro import Client

async def main():
    client = await Client.connect("tcp://127.0.0.1:7700")
    result = await client.call("math.add", [1, 2])
    print(result)  # 3
```

---

## Repository layout

```
Build/
  Cargo.toml          # Rust workspace root
  crates/             # Internal library crates
    saikuro-core/       Protocol types, envelope, error
    saikuro-schema/     Schema registry & validation
    saikuro-transport/  Transport backends
    saikuro-router/     Invocation router
    saikuro-runtime/    Embeddable runtime
    saikuro-codegen/    Binding code-generator
    saikuro-runtime-bin/ Standalone server binary
  tests/              # Rust integration tests
  adapters/
    rust/             Rust adapter (saikuro crate)
    typescript/       TypeScript/JS adapter
    python/           Python adapter
    csharp/           C# adapter
Docs/                 Documentation site
```

---

## Documentation

Full documentation is available at the project's GitHub Pages site.

---

## License

Apache-2.0. See [LICENSE](LICENSE).
