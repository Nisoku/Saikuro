---
title: "Rust Adapter"
description: "Native Rust provider and client APIs"
---

The Rust adapter is the native Saikuro adapter crate and exposes `Client` and `Provider`.

## Install

```toml
[dependencies]
saikuro = "0.1"
```

## Feature flags

| Feature | Description |
| ------- | ----------- |
| `tcp`   | TCP transport |
| `unix`  | Unix socket transport |
| `ws`    | WebSocket transport |

## Provider

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

## Client

```rust
use saikuro::{Client, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::connect("tcp://127.0.0.1:7700").await?;
    let result = client.call("math.add", vec![serde_json::json!(1), serde_json::json!(2)]).await?;
    println!("{result}");
    Ok(())
}
```

## Next Steps

- [Schema](../../guide/schema): Register metadata for validation and codegen
- [Code Generation](../../guide/codegen): Generate Rust bindings from frozen schema
- [Rust API Reference](./api-reference): Crate API surface reference
- [Rust examples](./examples): Rust-centered cross-language patterns