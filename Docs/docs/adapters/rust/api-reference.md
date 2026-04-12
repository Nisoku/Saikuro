---
title: "Rust API Reference"
description: "Reference for the Saikuro Rust adapter crate"
---

This page is the Rust adapter API reference. For shared protocol details, see [Core Protocol Reference](../../api/).

## Main types

- `Client`
- `ClientOptions`
- `SaikuroStream`
- `SaikuroChannel`
- `Provider`
- `Error`
- `Result<T>`

## Client API

| Symbol | Signature | Returns | Notes |
| ------ | --------- | ------- | ----- |
| `Client::connect` | `pub async fn connect(address: impl AsRef<str>)` | `Result<Client>` | Connect using address string. |
| `Client::connect_with_options` | `pub async fn connect_with_options(address, options)` | `Result<Client>` | Connect with `ClientOptions`. |
| `Client::close` | `pub async fn close(self)` | `Result<()>` | Graceful shutdown. |
| `Client::call` | `pub async fn call(&self, target, args)` | `Result<Value>` | Request/response invocation. |
| `Client::call_with_timeout` | `pub async fn call_with_timeout(&self, target, args, timeout)` | `Result<Value>` | Timed call variant. |
| `Client::cast` | `pub async fn cast(&self, target, args)` | `Result<()>` | Fire-and-forget invocation. |
| `Client::stream` | `pub async fn stream(&self, target, args)` | `Result<SaikuroStream>` | Server-to-client stream. |
| `Client::batch` | `pub async fn batch(&self, calls)` | `Result<Vec<Value>>` | Ordered per-call results. |
| `Client::channel` | `pub async fn channel(&self, target, args)` | `Result<SaikuroChannel>` | Bidirectional channel. |

## Provider API

| Symbol | Signature | Returns | Notes |
| ------ | --------- | ------- | ----- |
| `Provider::new` | `pub fn new(namespace: impl Into<String>)` | `Provider` | Namespace-scoped provider. |
| `Provider::namespace` | `pub fn namespace(&self)` | `&str` | Namespace getter. |
| `Provider::register` | `pub fn register(&mut self, name, handler)` | `()` | Register handler closure. |
| `Provider::register_with_options` | `pub fn register_with_options(&mut self, name, handler, options)` | `()` | Register with schema metadata. |
| `Provider::serve` | `pub async fn serve(self, address)` | `Result<()>` | Connect and serve loop. |
| `Provider::serve_on` | `pub async fn serve_on(self, transport)` | `Result<()>` | Serve on existing transport. |

## Error model

Adapter errors map to protocol error codes while preserving Rust-friendly error handling.

## Related

- [Rust Adapter Overview](./)
- [Rust Examples](./examples)
- [Core Protocol Reference](../../api/)