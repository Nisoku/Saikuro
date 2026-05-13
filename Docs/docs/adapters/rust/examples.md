---
title: "Rust Adapter Examples"
description: "Rust-centered cross-language patterns"
---

## Rust high-throughput provider

Use Rust for performance-sensitive services while callers stay in dynamic languages.

```rust
use saikuro::{Provider, Result};

fn main() -> Result<()> {
    saikuro_exec::block_on(async {
        let mut provider = Provider::new("index");
        provider.register("search", |args: Vec<serde_json::Value>| async move {
            Ok(run_search(args))
        });
        provider.serve("tcp://127.0.0.1:7700").await
    })
}
```

```python
client = Client()
await client.connect()
results = await client.call("index.search", ["saikuro"])
```

## Cross-language streaming fan-out

Rust emits events while TypeScript and Python consume independently.

```rust
provider.register_stream("subscribe", |filter: String| async move {
    stream_events(filter)
});
```

## Next Steps

- [Rust Adapter](./)
- [TypeScript examples](../typescript/examples)
- [Python examples](../python/examples)