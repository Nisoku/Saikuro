---
type: concept
title: "C++ API Reference"
description: "Full C++ adapter API reference"
source: "https://nisoku.org/Saikuro/adapters/cpp/api-reference/"
path: /adapters/cpp/api-reference/
updated: 2026-07-15
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-07-15T17:35:44.038Z"
---
---
title: "C++ API Reference"
description: "Full C++ adapter API reference"
---

The C++ adapter wraps the C API with RAII types in the `saikuro` namespace. All types are move-only.

## `saikuro::Client`

| Method                                                         | Description           |
|----------------------------------------------------------------|-----------------------|
| `Client(const std::string& address)`                           | Connect to a runtime  |
| `std::string call_json(target, args_json)`                     | Request/response call |
| `std::string call_json_timeout(target, args_json, timeout_ms)` | Call with timeout     |
| `void cast_json(target, args_json)`                            | Fire-and-forget       |
| `std::string batch_json(calls_json)`                           | Batch calls           |
| `Stream stream_json(target, args_json)`                        | Open a stream         |
| `Channel channel_json(target, args_json)`                      | Open a channel        |
| `std::string resource_json(target, args_json)`                 | Resource invocation   |
| `void log(level, name, msg, fields_json)`                      | Forward a log record  |

## `saikuro::Client::Stream`

| Method                             | Description                                 |
|------------------------------------|---------------------------------------------|
| `bool next_json(std::string& out)` | Read next item; returns `false` when closed |

## `saikuro::Client::Channel`

| Method                                    | Description                                 |
|-------------------------------------------|---------------------------------------------|
| `void send_json(const std::string& item)` | Send an item                                |
| `void close()`                            | Close the channel                           |
| `void abort()`                            | Abort the channel                           |
| `bool next_json(std::string& out)`        | Read next item; returns `false` when closed |

## `saikuro::Provider`

| Method                                                                             | Description          |
|------------------------------------------------------------------------------------|----------------------|
| `Provider(const std::string& namespace_name)`                                      | Create a provider    |
| `void register_handler(name, callback, user_data)`                                 | Register a handler   |
| `void register_handler_with_schema(name, callback, user_data, nargs, return_type)` | Register with schema |
| `void serve(const std::string& address)`                                           | Connect and serve    |

## Error Handling

Exceptions carry a descriptive message:

```cpp
try {
    saikuro::Client client("tcp://127.0.0.1:7700");
    auto result = client.call_json("math.add", "[1, 2]");
} catch (const saikuro::Error& e) {
    fprintf(stderr, "Saikuro error: %s\n", e.what());
}
```
