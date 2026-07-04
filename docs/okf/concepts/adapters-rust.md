---
type: concept
title: "Rust Adapter"
description: "Saikuro adapter for Rust"
source: "https://nisoku.org/Saikuro/adapters/rust/"
path: /adapters/rust/
updated: 2026-07-04
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-07-04T10:35:33.802Z"
---
---
title: "Rust Adapter"
description: "Saikuro adapter for Rust"
---

The Rust adapter provides async Client and Provider APIs, typed schema access, and integration with `saikuro-storage`.

## Installation

```toml
[dependencies]
saikuro = "0.1"
```

With storage:

```toml
saikuro = { version = "0.1", features = ["storage", "inmemory"] }
```

## Client API

```rust
use saikuro::{Client, Value};

let mut client = Client::connect("unix:///tmp/saikuro.sock").await?;

// Call
let result: i32 = client.call("math.add", &[1.into(), 2.into()]).await?;

// Cast
client.cast("log.write", &[serde_json::json!({"level": "info"}).into()]).await?;

// Stream
let mut stream = client.stream("events.subscribe", &[]).await?;
while let Some(value) = stream.next().await {
    println!("{value:?}");
}

// Batch
let results = client
    .batch(&[
        ("math.add", &[1.into(), 2.into()]),
        ("math.multiply", &[3.into(), 4.into()]),
    ])
    .await?;

client.close().await?;
```

## Provider API

```rust
use saikuro::{Provider, RegisterOptions};

let mut provider = Provider::new("math");

provider
    .register("add", |args: HandlerArgs| -> saikuro::Result<Value> {
        let a: i32 = args.get(0)?;
        let b: i32 = args.get(1)?;
        Ok((a + b).into())
    })
    .await?;

provider
    .register("divide", |args: HandlerArgs| -> saikuro::Result<Value> {
        let a: f64 = args.get(0)?;
        let b: f64 = args.get(1)?;
        if b == 0.0 {
            return Err(saikuro::Error::Provider("division by zero".into()));
        }
        Ok((a / b).into())
    })
    .with_options(RegisterOptions {
        capabilities: vec!["math.divide".into()],
        ..Default::default()
    })
    .await?;

provider.serve("unix:///tmp/saikuro.sock").await?;
```

## Storage

```rust
use saikuro::storage::{create_storage, KeyValueBackend, KeyValueBackendExt};

let storage = create_transient_storage()?;
storage.put("cache", "key", b"value".into()).await?;
let val = storage.get("cache", "key").await?;
```

## Export Surface

```rust
// Core
Client, Provider, SaikuroStream, SaikuroChannel
ClientOptions, RegisterOptions

// Schema
Value, TypeDescriptor, PrimitiveType
FunctionSchema, NamespaceSchema
ArgDescriptor, HandlerArgs

// Transport
InMemoryTransport

// Storage (feature-gated)
create_storage, create_transient_storage

// Errors
Error, Result
```
