---
title: "C++ Adapter"
description: "RAII C++ wrapper over the Saikuro C ABI"
---

The C++ adapter wraps the C ABI in RAII-style classes for safer lifetime management.

## Header

- `Build/adapters/cpp/include/saikuro/saikuro.hpp`

## Main types

- `saikuro::Client`
- `saikuro::Client::Stream`
- `saikuro::Client::Channel`
- `saikuro::Provider`
- `saikuro::Error`

## Helper APIs

- `batch_json(...)`
- `stream_json(...)`
- `channel_json(...)`
- `resource_json(...)`
- `log(...)`

## Capability parity

| Primitive | C++ adapter support |
| --------- | ------------------- |
| `call` | Yes |
| `cast` | Yes |
| `stream` | Yes |
| `channel` | Yes |
| `batch` | Yes |
| `resource` | Yes |

## Dependency

The C++ adapter depends on the C adapter artifacts:

- Header: `Build/adapters/c/include/saikuro.h`
- Library: `saikuro_c`

## Lifetime model

The C++ layer is designed as an ownership-safe RAII wrapper over the C ABI. Prefer wrapper APIs to avoid manual string and handle lifetime management.

## Schema extractor CLI

The C++ adapter includes `saikuro-cpp-schema`, a header-based schema extractor.

```bash
saikuro-cpp-schema --namespace parityns ./service.h
saikuro-cpp-schema --namespace parityns --pretty ./service.h
```

The parser targets common semicolon-terminated function prototypes and warns when signatures are too complex to parse safely.

## Next Steps

- [C Adapter](../c/): ABI-level contract and ownership rules
- [C++ API Reference](./api-reference): Wrapper classes and method reference
- [C++ examples](./examples): RAII usage patterns
- [Schema](../../guide/schema): Function/type model that extractors emit