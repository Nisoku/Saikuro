---
type: concept
title: "C API Reference"
description: "Full C adapter API reference"
source: "https://nisoku.org/Saikuro/adapters/c/api-reference/"
path: /adapters/c/api-reference/
updated: 2026-06-27
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-06-27T13:11:26.465Z"
---
---
title: "C API Reference"
description: "Full C adapter API reference"
---

## String Helpers

| Function                                      | Description                                                               |
|-----------------------------------------------|---------------------------------------------------------------------------|
| `char* saikuro_string_dup(const char* input)` | Allocate a copy of `input` (caller owns, free with `saikuro_string_free`) |
| `void saikuro_string_free(char* ptr)`         | Free a string returned by the Saikuro API                                 |

## Client

| Function                                                                        | Description                       |
|---------------------------------------------------------------------------------|-----------------------------------|
| `saikuro_client_t saikuro_client_connect(const char* address)`                  | Connect to a runtime at `address` |
| `int saikuro_client_close(saikuro_client_t handle)`                             | Close the connection              |
| `void saikuro_client_free(saikuro_client_t handle)`                             | Free the client handle            |
| `char* saikuro_client_call_json(handle, target, args_json)`                     | Request/response call             |
| `char* saikuro_client_call_json_timeout(handle, target, args_json, timeout_ms)` | Call with timeout                 |
| `int saikuro_client_cast_json(handle, target, args_json)`                       | Fire-and-forget                   |
| `char* saikuro_client_batch_json(handle, calls_json)`                           | Batch calls                       |
| `saikuro_stream_t saikuro_client_stream_json(handle, target, args_json)`        | Open a stream                     |
| `saikuro_channel_t saikuro_client_channel_json(handle, target, args_json)`      | Open a channel                    |
| `char* saikuro_client_resource_json(handle, target, args_json)`                 | Resource invocation               |
| `int saikuro_client_log(handle, level, name, msg, fields_json)`                 | Forward a log record              |

Stream results and channel items are heap-allocated C strings. Free with `saikuro_string_free`.

## Stream

| Function                                                          | Description                                   |
|-------------------------------------------------------------------|-----------------------------------------------|
| `int saikuro_stream_next_json(stream, &out_item_json, &out_done)` | Read next item; sets `out_done=1` when closed |
| `void saikuro_stream_free(stream)`                                | Free the stream handle                        |

## Channel

| Function                                                            | Description             |
|---------------------------------------------------------------------|-------------------------|
| `int saikuro_channel_send_json(channel, item_json)`                 | Send an item            |
| `int saikuro_channel_close(channel)`                                | Close the channel       |
| `int saikuro_channel_abort(channel)`                                | Abort the channel       |
| `int saikuro_channel_next_json(channel, &out_item_json, &out_done)` | Read next item          |
| `void saikuro_channel_free(channel)`                                | Free the channel handle |

## Provider

| Function                                                                                                | Description              |
|---------------------------------------------------------------------------------------------------------|--------------------------|
| `saikuro_provider_t saikuro_provider_new(const char* namespace_name)`                                   | Create a provider        |
| `int saikuro_provider_register(handle, name, callback, user_data)`                                      | Register a handler       |
| `int saikuro_provider_register_with_schema(handle, name, callback, user_data, nargs, return_type_json)` | Register with type info  |
| `int saikuro_provider_serve(handle, address)`                                                           | Connect and serve        |
| `int saikuro_provider_close(handle)`                                                                    | Close the provider       |
| `void saikuro_provider_free(handle)`                                                                    | Free the provider handle |

Error information is thread-local. Retrieve it with `saikuro_last_error_message()` (free with `saikuro_string_free`).
