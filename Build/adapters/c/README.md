# saikuro C adapter

C adapter for the Saikuro cross-language IPC fabric.

This adapter exposes a stable C ABI and is implemented on top of the Rust
adapter.

## Build

```bash
cd Build
cargo build -p saikuro-c
```

This produces a static and dynamic library (`libsaikuro_c.a` and `libsaikuro_c.so`
on Linux) under Cargo target directories.

## Header

Public C API header:

- `include/saikuro.h`

## API model

- `saikuro_client_t`: connect and call/cast/batch/resource using JSON argument arrays.
- `saikuro_stream_t`: consume stream items as JSON.
- `saikuro_channel_t`: bidirectional JSON channel send/receive.
- `saikuro_provider_t`: register callbacks and serve a namespace.

Client helper APIs include:

- `saikuro_client_batch_json(...)` for multi-call batch invocations.
- `saikuro_client_stream_json(...)` + `saikuro_stream_next_json(...)` for stream consumption.
- `saikuro_client_channel_json(...)` + `saikuro_channel_send_json(...)`/`saikuro_channel_next_json(...)` for channels.
- `saikuro_client_resource_json(...)` for resource invocations.
- `saikuro_client_log(...)` for structured runtime log forwarding.

Provider callbacks receive JSON `args` and return a JSON result string.

Important: callback return strings must be allocated using `saikuro_string_dup`
so that ownership and freeing are well-defined across the ABI boundary.

## Error handling

Functions that fail return `NULL`/non-zero.

Use:

- `saikuro_last_error_message()` to fetch the latest error as a heap string.
- `saikuro_string_free()` to release returned error/result strings.

## License

Apache-2.0
