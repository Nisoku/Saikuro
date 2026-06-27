---
type: concept
title: "Error Handling"
description: "How Saikuro surfaces and handles errors across languages"
source: "https://nisoku.org/Saikuro/guide/errors/"
path: /guide/errors/
updated: 2026-06-27
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-06-27T14:04:05.297Z"
---
---
title: "Error Handling"
description: "How Saikuro surfaces and handles errors across languages"
---

When an invocation fails, the runtime returns an `ErrorDetail` payload inside the response envelope. Each adapter maps this to a native exception or error type.

## ErrorDetail

Every failed response carries a machine-readable code, a human-readable message, and optional structured details:

```json
{
  "id": "<uuid>",
  "ok": false,
  "error": {
    "code": "FunctionNotFound",
    "message": "No handler registered for 'math.divide'",
    "details": { "target": "math.divide" }
  }
}
```

## Error Codes

The runtime defines these error categories:

| Code                   | Meaning                                                      |
|------------------------|--------------------------------------------------------------|
| `NamespaceNotFound`    | The requested namespace is not registered                    |
| `FunctionNotFound`     | The requested function does not exist in its namespace       |
| `InvalidArguments`     | Arguments failed type or shape validation                    |
| `IncompatibleVersion`  | Envelope protocol version is incompatible with the runtime   |
| `MalformedEnvelope`    | A required field was missing from an envelope                |
| `NoProvider`           | No provider is registered for the target namespace           |
| `ProviderUnavailable`  | The provider is temporarily unavailable                      |
| `BatchRoutingConflict` | A batch item's target resolved to a different namespace      |
| `CapabilityDenied`     | The caller lacks a required capability token                 |
| `CapabilityInvalid`    | The capability token presented is invalid or expired         |
| `ConnectionLost`       | The underlying transport connection was lost                 |
| `MessageTooLarge`      | A message exceeded the configured size limit (16 MiB)        |
| `Timeout`              | The operation exceeded its timeout                           |
| `BufferOverflow`       | Receive buffer overflowed due to backpressure violation      |
| `ProviderError`        | The provider handler returned an explicit error              |
| `ProviderPanic`        | The provider panicked while handling the invocation          |
| `StreamClosed`         | Stream was already closed when an item was sent              |
| `ChannelClosed`        | Channel was closed by the remote side                        |
| `OutOfOrder`           | Out-of-order sequence number on an ordered stream or channel |
| `Internal`             | An error category not covered by the above codes             |

## Adapter Error Handling

Each adapter maps `ErrorDetail` to a native error type:

```typescript
// TypeScript
import { SaikuroClient, SaikuroTimeoutError } from "@nisoku/saikuro";

try {
  const result = await client.call("math.add", [1, 2], { timeoutMs: 1000 });
} catch (err) {
  if (err instanceof SaikuroTimeoutError) {
    console.error("call timed out");
  } else if (err instanceof FunctionNotFoundError) {
    console.error("function not found");
  }
}
```

```python
# Python
from saikuro import SaikuroError, TransportError

try:
    result = await client.call("math.add", [1, 2])
except TransportError as e:
    print(f"connection failed: {e}")
except SaikuroError as e:
    print(f"saikuro error: {e}")
```

```rust
// Rust
use saikuro::Error;

match client.call::<i32>("math.add", &[1.into(), 2.into()]).await {
    Ok(result) => println!("{result}"),
    Err(Error::Timeout { millis }) => eprintln!("timed out after {millis}ms"),
    Err(e) => eprintln!("error: {e}"),
}
```

```csharp
// C#
try {
    int result = await client.CallAsync<int>("math.add", new object[] { 1, 2 });
} catch (SaikuroTimeoutException ex) {
    Console.WriteLine("call timed out");
} catch (SaikuroException ex) {
    Console.WriteLine($"saikuro error: {ex.Message}");
}
```

```c
// C
char* err = saikuro_last_error_message();
if (err) {
    fprintf(stderr, "Saikuro error: %s\n", err);
    saikuro_string_free(err);
}
```

```cpp
// C++
try {
    auto result = client.call_json("math.add", "[1, 2]");
} catch (const saikuro::Error& e) {
    fprintf(stderr, "Saikuro error: %s\n", e.what());
}
```

## Provider Error Handling

Providers signal errors by throwing or returning an error:

```typescript
// TypeScript provider
provider.register("divide", (a: number, b: number) => {
  if (b === 0) throw new Error("division by zero");
  return a / b;
});
```

```python
# Python provider
@provider.register("divide")
def divide(a: float, b: float) -> float:
    if b == 0:
        raise ValueError("division by zero")
    return a / b
```

```rust
// Rust provider
provider
    .register("divide", |args: HandlerArgs| -> saikuro::Result<Value> {
        let b: f64 = args.get(1)?;
        if b == 0.0 {
            return Err(saikuro::Error::Provider("division by zero".into()));
        }
        Ok((a / b).into())
    })
    .await?;
```

The runtime catches the error and returns it to the caller as an `ProviderError` code with the message preserved.

### Panics

If a provider panics (uncaught exception in Python, panic in Rust, unhandled rejection in TypeScript), the runtime catches it and returns `ProviderPanic`. The provider process remains running and can handle subsequent calls.

## Timeouts

Calls can specify a timeout in milliseconds. If the provider does not respond in time, the runtime returns `Timeout`:

```typescript
const result = await client.call("math.add", [1, 2], { timeoutMs: 5000 });
```

```python
result = await client.call("math.add", [1, 2], timeout=5.0)
```

```csharp
using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(5));
var result = await client.CallAsync<int>("math.add", new object[] { 1, 2 }, cts.Token);
```

## Next Steps

::: grids
::: grid
::: button "Structured Logging" ./logging.md icon:terminal
:::
::: grid
::: button "Invocation Primitives" ./invocations.md icon:zap
:::
::: grid
::: button "Protocol Reference" ../api/ icon:box
:::
:::
