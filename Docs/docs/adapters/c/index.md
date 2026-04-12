---
title: "C Adapter"
description: "Stable C ABI for Saikuro"
---

The C adapter provides a stable C ABI over the Rust adapter runtime.

## Build

```bash
cd Build
cargo build -p saikuro-c
```

Output libraries are produced in Cargo target directories (for example `libsaikuro_c.a` and `libsaikuro_c.so` on Linux).

## Header

- `Build/adapters/c/include/saikuro.h`

## API model

- `saikuro_client_t`: connect, call, cast, batch, stream, channel, and resource invocation
- `saikuro_stream_t`: read stream items as JSON
- `saikuro_channel_t`: send/receive channel JSON frames
- `saikuro_provider_t`: register C callbacks and serve a namespace

## Capability parity

| Primitive | C adapter support |
| --------- | ----------------- |
| `call` | Yes (`saikuro_client_call_json`) |
| `cast` | Yes (`saikuro_client_cast_json`) |
| `stream` | Yes (`saikuro_client_stream_json`) |
| `channel` | Yes (`saikuro_client_channel_json`) |
| `batch` | Yes (`saikuro_client_batch_json`) |
| `resource` | Yes (`saikuro_client_resource_json`) |

## Ownership and safety

- Pointer-returning APIs return `NULL` on failure
- Integer-returning APIs return non-zero on failure
- Use `saikuro_last_error_message()` for diagnostics
- Free adapter-owned returned strings with `saikuro_string_free()`
- Provider callbacks must return heap-allocated JSON strings created with `saikuro_string_dup()`

## Ownership-safe callback example

```c
static char *sum_cb(void *ctx, const char *args_json) {
	(void)ctx;
	(void)args_json;
	return saikuro_string_dup("42");
}
```

The callback return value is owned by the adapter runtime after return. Do not return stack pointers or string literals.

## Useful APIs

- `saikuro_client_batch_json(...)`
- `saikuro_client_stream_json(...)` + `saikuro_stream_next_json(...)`
- `saikuro_client_channel_json(...)` + `saikuro_channel_send_json(...)`
- `saikuro_client_resource_json(...)`
- `saikuro_client_log(...)`

## Next Steps

- [C++ Adapter](../cpp/): RAII wrapper over the C ABI
- [C API Reference](./api-reference): Stable ABI function and handle reference
- [C examples](./examples): C ABI usage and ownership patterns
- [Code Generation](../../guide/codegen): Generate C-compatible clients from schema