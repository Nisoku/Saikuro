---
title: "C++ API Reference"
description: "Reference for the Saikuro C++ wrapper API"
---

This page is the C++ adapter API reference. For shared protocol details, see [Core Protocol Reference](../../api/).

## Main classes

- `saikuro::Client`
- `saikuro::Client::Stream`
- `saikuro::Client::Channel`
- `saikuro::Provider`
- `saikuro::Error`

## Client wrapper API

| Symbol | Signature | Returns | Notes |
| ------ | --------- | ------- | ----- |
| `Client` | `explicit Client(const std::string& address)` | `Client` | Connect on construction. |
| `Client::call_json` | `std::string call_json(const std::string& target, const std::string& args_json) const` | `std::string` | Throws `saikuro::Error` on failure. |
| `Client::call_json_timeout` | `std::string call_json_timeout(const std::string& target, const std::string& args_json, int timeout_ms) const` | `std::string` | Timed call variant. |
| `Client::cast_json` | `void cast_json(const std::string& target, const std::string& args_json) const` | `void` | Fire-and-forget invocation. |
| `Client::batch_json` | `std::string batch_json(const std::string& calls_json) const` | `std::string` | Batch invocation result JSON. |
| `Client::stream_json` | `Stream stream_json(const std::string& target, const std::string& args_json) const` | `Stream` | Opens server-to-client stream. |
| `Client::channel_json` | `Channel channel_json(const std::string& target, const std::string& args_json) const` | `Channel` | Opens bidirectional channel. |
| `Client::resource_json` | `std::string resource_json(const std::string& target, const std::string& args_json) const` | `std::string` | Resource invocation result JSON. |
| `Client::log` | `void log(const std::string& level, const std::string& name, const std::string& msg, const std::string& fields_json="{}") const` | `void` | Structured runtime log forwarding. |

## Stream and channel wrappers

| Symbol | Signature | Returns | Notes |
| ------ | --------- | ------- | ----- |
| `Client::Stream::next_json` | `bool next_json(std::string& out_item_json)` | `bool` | `false` when stream is done. |
| `Client::Channel::send_json` | `void send_json(const std::string& item_json)` | `void` | Send one channel item. |
| `Client::Channel::next_json` | `bool next_json(std::string& out_item_json)` | `bool` | `false` when channel is done. |
| `Client::Channel::close` | `void close()` | `void` | Graceful channel close. |
| `Client::Channel::abort` | `void abort()` | `void` | Abort channel. |

## Provider wrapper API

| Symbol | Signature | Returns | Notes |
| ------ | --------- | ------- | ----- |
| `Provider` | `explicit Provider(const std::string& namespace_name)` | `Provider` | Creates namespace provider wrapper. |
| `Provider::register_handler` | `void register_handler(const std::string& name, RawHandler callback, void* user_data)` | `void` | Registers C callback bridge. |
| `Provider::serve` | `void serve(const std::string& address)` | `void` | Blocking serve loop. |

## Error model

Adapter failures map to `saikuro::Error` with code/message semantics aligned to the core protocol.

## Header location

- `Build/adapters/cpp/include/saikuro/saikuro.hpp`

## Related

- [C++ Adapter Overview](./)
- [C++ Examples](./examples)
- [Core Protocol Reference](../../api/)