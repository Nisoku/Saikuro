---
title: "C API Reference"
description: "Reference for the stable Saikuro C ABI"
---

This page is the C adapter API reference. For shared protocol details, see [Core Protocol Reference](../../api/).

## Core handles

- `saikuro_client_t`
- `saikuro_stream_t`
- `saikuro_channel_t`
- `saikuro_provider_t`

## String and error utilities

| Symbol | Signature | Returns | Notes |
| ------ | --------- | ------- | ----- |
| `saikuro_string_dup` | `char* saikuro_string_dup(const char* input)` | `char*` | Heap copy; free with `saikuro_string_free`. |
| `saikuro_string_free` | `void saikuro_string_free(char* ptr)` | `void` | Frees adapter-owned strings. |
| `saikuro_last_error_message` | `char* saikuro_last_error_message(void)` | `char*` | Thread-local last error snapshot. |

## Client API

| Symbol | Signature | Returns | Notes |
| ------ | --------- | ------- | ----- |
| `saikuro_client_connect` | `saikuro_client_t saikuro_client_connect(const char* address)` | `saikuro_client_t` | Returns `NULL` on failure. |
| `saikuro_client_close` | `int saikuro_client_close(saikuro_client_t handle)` | `int` | Non-zero on failure. |
| `saikuro_client_free` | `void saikuro_client_free(saikuro_client_t handle)` | `void` | Releases client handle. |
| `saikuro_client_call_json` | `char* saikuro_client_call_json(handle, target, args_json)` | `char*` | JSON result string, ownership transferred. |
| `saikuro_client_call_json_timeout` | `char* saikuro_client_call_json_timeout(handle, target, args_json, timeout_ms)` | `char*` | Timed call variant. |
| `saikuro_client_cast_json` | `int saikuro_client_cast_json(handle, target, args_json)` | `int` | Fire-and-forget invocation. |
| `saikuro_client_batch_json` | `char* saikuro_client_batch_json(handle, calls_json)` | `char*` | Batch result JSON. |
| `saikuro_client_stream_json` | `saikuro_stream_t saikuro_client_stream_json(handle, target, args_json)` | `saikuro_stream_t` | Stream handle or `NULL`. |
| `saikuro_client_channel_json` | `saikuro_channel_t saikuro_client_channel_json(handle, target, args_json)` | `saikuro_channel_t` | Channel handle or `NULL`. |
| `saikuro_client_resource_json` | `char* saikuro_client_resource_json(handle, target, args_json)` | `char*` | Resource invocation JSON result. |
| `saikuro_client_log` | `int saikuro_client_log(handle, level, name, msg, fields_json)` | `int` | Structured runtime log forwarding. |

## Stream and channel API

| Symbol | Signature | Returns | Notes |
| ------ | --------- | ------- | ----- |
| `saikuro_stream_next_json` | `int saikuro_stream_next_json(stream, char** out_item_json, int* out_done)` | `int` | `out_done=1` signals end/error. |
| `saikuro_stream_free` | `void saikuro_stream_free(saikuro_stream_t stream)` | `void` | Releases stream handle. |
| `saikuro_channel_send_json` | `int saikuro_channel_send_json(channel, item_json)` | `int` | Send one channel item. |
| `saikuro_channel_next_json` | `int saikuro_channel_next_json(channel, char** out_item_json, int* out_done)` | `int` | Receive next channel item. |
| `saikuro_channel_close` | `int saikuro_channel_close(saikuro_channel_t channel)` | `int` | Graceful close. |
| `saikuro_channel_abort` | `int saikuro_channel_abort(saikuro_channel_t channel)` | `int` | Abort channel. |
| `saikuro_channel_free` | `void saikuro_channel_free(saikuro_channel_t channel)` | `void` | Releases channel handle. |

## Provider API

| Symbol | Signature | Returns | Notes |
| ------ | --------- | ------- | ----- |
| `saikuro_provider_new` | `saikuro_provider_t saikuro_provider_new(const char* namespace_name)` | `saikuro_provider_t` | Returns `NULL` on failure. |
| `saikuro_provider_register` | `int saikuro_provider_register(handle, name, callback, user_data)` | `int` | Callback must return owned heap string. |
| `saikuro_provider_serve` | `int saikuro_provider_serve(handle, address)` | `int` | Blocking serve loop. |
| `saikuro_provider_free` | `void saikuro_provider_free(saikuro_provider_t handle)` | `void` | Releases provider handle. |

## Error and memory conventions

- Pointer-returning functions: `NULL` means failure
- Integer-returning functions: non-zero means failure
- Last error: `saikuro_last_error_message()`
- Free returned strings with `saikuro_string_free(...)`
- Provider callbacks must return heap strings created with `saikuro_string_dup(...)`

## Header location

- `Build/adapters/c/include/saikuro.h`

## Related

- [C Adapter Overview](./)
- [C Examples](./examples)
- [Core Protocol Reference](../../api/)