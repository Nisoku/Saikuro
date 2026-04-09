# Changelog

All notable changes to Saikuro will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

---

## [Uncommited-0.1.0] - 2026-03-05 - 2026-03-06

### Added

- `saikuro-runtime-bin`: production-ready standalone runtime server binary with `--schema`, `--tcp-port`, `--ws-port`, `--unix`, `--mode`, `--log-level`, `--json-logs` CLI flags and graceful shutdown on SIGTERM/SIGINT
- Rust adapter (`Build/adapters/rust/`): `Provider` and `Client` with TCP, WebSocket, and Unix socket transport support, schema announcement, and batch/stream/cast invocation types
- C# codegen generator (`saikuro-codegen::csharp`): produces `Types.cs`, per-namespace `<Namespace>Client.cs`, and `Generated.cs` index
- Integration test suite covering C# codegen output (11 new tests; 33 codegen tests total across TypeScript, Python, C#)
- `idempotent` and `doc` fields documented in `Docs/docs/guide/schema.md`
- `Docs/docs/guide/codegen.md`: full code generation guide covering TypeScript, Python, and C# output with examples
- bunch more stuff but i'm tired of writing all fancy, it takes too much mental effort and I finally finished everything

### Fixed

- Python adapter version requirement corrected from `>=3.10` to `>=3.11` in `quickstart.md` and `adapters.md`
- TypeScript `package.json` and Python `pyproject.toml` license field corrected to `Apache-2.0`
- Rust adapter `adapters.md` section updated to match actual `Vec<serde_json::Value>` API

---

## [Uncommited-0.0.9] - 2026-02-24

### Added

- `saikuro-codegen` crate: `BindingGenerator` trait, `TypeScriptGenerator`, and `PythonGenerator` with per-namespace client stubs, shared types file, and index/init re-export
- Integration tests for TypeScript and Python codegen output: visibility filtering, primitive type mapping, stream/channel method names, enum and record type emission
- `cross_language_wire` integration test: TypeScript and Python adapter wire-format parity verified against the Rust runtime in-process

### Changed

- Schema `visibility` field is now enforced during codegen; private functions are excluded from all generated output
- `saikuro-schema` validator rejects function calls targeting private functions from external transports

---

## [Uncommited-0.0.8] - 2026-02-14

### Added

- `sandbox_dispatch` integration test: provider-side panics are caught and returned as `ProviderError` responses rather than crashing the runtime
- `log_dispatch` integration test: structured log messages from providers are forwarded to the runtime log stream
- `resource_dispatch` integration test: resource handle lifecycle (acquire, use, release) across the transport boundary
- C# adapter: `SchemaExtractor` for dev-mode schema announcement, `Logger` structured log forwarding, resource handle support

### Fixed

- `channel_dispatch`: channel `Close` frame was not being forwarded when the provider closed its end first
- `saikuro-transport` WebSocket receiver: oversized frames no longer panic; they return a `FrameTooLarge` error instead

---

## [Uncommited-0.0.7] - 2026-02-04

### Added

- `channel_dispatch` integration test covering bidirectional message exchange and clean shutdown from both ends
- C# adapter `Client.ChannelAsync` and `Provider.RegisterChannel` with `IAsyncEnumerable` receive loop
- TypeScript adapter `client.channel()` and `provider.registerChannel()` with async generator receive loop
- Python adapter `client.channel()` context manager and `provider.register_channel()` async generator handler

### Changed

- `Envelope` now carries an optional `seq` field for ordering stream and channel frames
- `saikuro-router` stream state machine tracks sequence numbers and drops out-of-order frames with a warning

---

## [Uncommited-0.0.6] - 2026-01-27

### Added

- `stream_dispatch` integration test: server-to-client streaming with `StreamControl::End` and `StreamControl::Abort`
- TypeScript adapter `client.stream()` returns an `AsyncIterable`
- Python adapter `client.stream()` returns an `AsyncGenerator`
- C# adapter `client.StreamAsync<T>()` returns `IAsyncEnumerable<T>`
- `saikuro-core` `StreamControl` enum variants: `Chunk`, `End`, `Abort`

### Fixed

- `saikuro-router` was not cleaning up stream state on `Abort`; state entries now always removed on any terminal control frame

---

## [Uncommited-0.0.5] - 2026-01-20

### Added

- `capability_enforcement` integration test: calls missing required capability tokens are rejected with `CapabilityDenied` before reaching the provider
- `saikuro-schema` `CapabilityEngine`: validates caller token set against function-level required capabilities
- `error_propagation` integration test: provider-returned errors preserve `code` and `message` across the wire
- TypeScript `SaikuroError` class with typed `code` field
- Python `SaikuroError` exception with `code` and `message` attributes
- C# `SaikuroException` with `Code` property

### Changed

- `saikuro-runtime` connection handler now runs capability check before dispatching to the router

---

## [Uncommited-0.0.4] - 2026-01-15

### Added

- `batch_dispatch` integration test covering multi-call batch envelopes
- `schema_validation` integration test: malformed envelopes and unknown function targets return structured errors
- `announce_dispatch` integration test: provider schema announcement flow end-to-end
- Python adapter tests: `test_client`, `test_provider`, `test_envelope`, `test_transport`, `test_error`
- TypeScript adapter tests: `client.test.ts`, `provider.test.ts`, `envelope.test.ts`, `transport.test.ts`, `error.test.ts`

### Fixed

- `saikuro-transport` TCP framing: length-prefix was written as little-endian; changed to big-endian to match spec
- Python adapter was not stripping the length prefix on recv; now consistent with the Rust implementation

---

## [Uncommited-0.0.3] - 2026-01-11

### Added

- `call_dispatch` integration test: end-to-end request/response call through the in-memory transport
- `envelope_roundtrip` integration test: all envelope types serialize and deserialize correctly via MessagePack
- `in_memory_transport` integration test
- `saikuro-transport` in-memory transport pair for testing without network I/O
- C# adapter initial implementation: `Provider`, `Client`, `Envelope`, `Transport`, `Errors`
- C# adapter tests: `ProviderTests`, `ClientTests`, `EnvelopeTests`, `TransportTests`, `ErrorTests`

### Changed

- `saikuro-core` `Value` enum: removed `Unit` variant; null/absent values use `Value::Null`
- `saikuro-core` `ErrorCode`: removed `Unknown` variant; catch-all is now `ErrorCode::Internal`

---

## [Uncommited-0.0.2] - 2026-01-09

### Added

- `saikuro-schema` crate: schema registry, capability engine, and request validator
- `saikuro-router` crate: provider registry and request dispatch loop
- `saikuro-runtime` crate: `SaikuroRuntime`, `RuntimeHandle`, `RuntimeConfig`, and `RuntimeMode`
- TypeScript adapter initial implementation: `Provider`, `Client`, `Envelope`, `Transport`
- Python adapter initial implementation: `Provider`, `Client`, `Envelope`, `Transport`
- `saikuro-transport` Unix domain socket transport

### Fixed

- `saikuro-transport` WebSocket transport: client connect now correctly upgrades the TCP stream

---

## [Uncommited-0.0.1] - 2026-01-08

### Added

- `saikuro-core` crate: `Envelope`, `ResponseEnvelope`, `Value`, `ErrorCode`, `ErrorDetail`, `InvocationId`, `CapabilityToken`, `Schema`, `FunctionSchema`, `NamespaceSchema`, `ArgumentDescriptor`
- `saikuro-transport` crate: framing layer, TCP transport, WebSocket transport, `TransportSender`/`TransportReceiver` traits
- Rust workspace at `Build/Cargo.toml` with resolver 2, shared `[workspace.package]` metadata, and `[workspace.dependencies]`
- Initial `Docs/` site scaffold with VitePress config, getting-started and guide sections
- Apache-2.0 `LICENSE`
