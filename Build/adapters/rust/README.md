# saikuro Rust adapter

Rust adapter for the [Saikuro](https://github.com/Nisoku/Saikuro) cross-language
IPC fabric. Provides [`Client`] and [`Provider`]: the two building blocks for
writing Rust services that connect to a Saikuro runtime.

## Installation

```toml
[dependencies]
saikuro = "0.1"
saikuro-exec = "0.1"
```

Feature flags (all enabled by default):

| Feature | Description           |
| ------- | --------------------- |
| `tcp`   | TCP transport         |
| `unix`  | Unix socket transport |
| `ws`    | WebSocket transport   |

## Usage

### Provider

```rust
use saikuro::{Provider, Result};

fn main() -> Result<()> {
    saikuro_exec::block_on(async {
        let mut provider = Provider::new("math");

        provider.register("add", |args: Vec<serde_json::Value>| async move {
            let a = args[0].as_i64().unwrap_or(0);
            let b = args[1].as_i64().unwrap_or(0);
            Ok(serde_json::json!(a + b))
        });

        provider.serve("tcp://127.0.0.1:7700").await
    })
}
```

### Client

```rust
use saikuro::{Client, Result};

fn main() -> Result<()> {
    saikuro_exec::block_on(async {
        let client = Client::connect("tcp://127.0.0.1:7700").await?;

        let result = client.call("math.add", vec![
            serde_json::json!(1),
            serde_json::json!(2),
        ]).await?;

        println!("{result}");
        Ok(())
    })
}
```

## License

Apache-2.0
