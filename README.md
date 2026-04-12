# Saikuro

A language-agnostic, easy-to-use IPC library for cross-language integration.

Saikuro lets you expose functions written in one language and call them
transparently from any other supported language, with a shared typed schema,
capability enforcement, and pluggable transports (TCP, Unix socket, WebSocket,
or in-memory).

---

## Supported languages

(Saikuro currently is not added to package managers yet. It will be added soon, once I finalize a few things!)

| Language   | Package name | Status |
|------------|--------------|--------|
| Rust       | (TBD)        | ✅     |
| TypeScript | (TBD)        | ✅     |
| Python     | (TBD)        | ✅     |
| C#         | (TBD)        | ✅     |
| C          | (TBD)        | ✅     |
| C++        | (TBD)        | ✅     |

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

```txt
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

Full documentation is available at the project's GitHub Pages site:

- https://nisoku.github.io/Saikuro/docs/
- Adapter docs index: https://nisoku.github.io/Saikuro/docs/adapters/

Repository docs sources:

- `Docs/docs/index.md`
- `Docs/docs/adapters/index.md`
- `Docs/docs/guide/`

---

## Contributing

Contributions are welcome! Here’s how to get started:

1. Fork the repository.
2. Create a branch for your feature, adapter, or bug fix (`git checkout -b feature/name`).
3. Commit your changes (`git commit -m "Description of change"`).
4. Push your branch (`git push origin feature/name`).
5. Open a Pull Request.

Please make sure your changes follow the existing style and that all tests pass before submitting.

See [CONTRIBUTING](CONTRIBUTING.md) for more details.

---

## Star History

<a href="https://www.star-history.com/?repos=Nisoku%2FSaikuro&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/image?repos=Nisoku/Saikuro&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/image?repos=Nisoku/Saikuro&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/image?repos=Nisoku/Saikuro&type=date&legend=top-left" />
 </picture>
</a>

---

## License

Apache-2.0. See [LICENSE](LICENSE).
