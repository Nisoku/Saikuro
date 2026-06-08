---
title: "Rust Examples"
description: "Rust adapter usage patterns"
---

## Provider with Storage

```rust
use saikuro::{Provider, Client, Value};
use saikuro::storage::{create_transient_storage, KeyValueBackend};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut provider = Provider::new("kv");

    provider
        .register("get", |args| {
            let key: String = args.get(0)?;
            let storage = create_transient_storage()?;
            let val = storage.get("kv", &key).await?;
            Ok(val.map(|v| Value::Bytes(v.to_vec())).unwrap_or(Value::Unit))
        })
        .await?;

    provider.serve("unix:///tmp/saikuro.sock").await?;
    Ok(())
}
```

## Testing

```rust
use saikuro::{Client, Provider, InMemoryTransport};

#[tokio::test]
async fn test_math() {
    let (pt, ct) = InMemoryTransport::pair();

    let mut provider = Provider::new("math");
    provider
        .register("add", |args| {
            let a: i32 = args.get(0)?;
            let b: i32 = args.get(1)?;
            Ok((a + b).into())
        })
        .await
        .unwrap();
    tokio::spawn(async move { provider.serve_on(pt).await });

    let mut client = Client::open_on(ct).await.unwrap();
    let result: i32 = client.call("math.add", &[1.into(), 2.into()]).await.unwrap();
    assert_eq!(result, 3);
}
```
