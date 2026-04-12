# saikuro C++ adapter

C++ adapter for Saikuro built as a wrapper over the C adapter ABI.

## Header

- `include/saikuro/saikuro.hpp`

The header wraps:

- `saikuro::Client`
- `saikuro::Client::Stream`
- `saikuro::Client::Channel`
- `saikuro::Provider`
- `saikuro::Error`

Client helper APIs include `batch_json(...)`, `stream_json(...)`, `channel_json(...)`,
`resource_json(...)`, and `log(...)`.

## Schema extractor CLI

The C++ adapter includes a header-based schema extractor CLI:

- `saikuro-cpp-schema`

Usage:

```bash
saikuro-cpp-schema --namespace parityns ./service.h
saikuro-cpp-schema --namespace parityns --pretty ./service.h
```

The schema extractor uses a regex-based parser aimed at common, simple
semicolon-terminated function prototypes. It supports declarations like:

- `int add(int a, int b);`
- `const char* echo(const char* msg);`

Known limitations:

- Function-pointer parameters may be skipped.
- Complex nested template signatures with embedded commas may be skipped.

When these patterns are detected, the extractor emits a warning to stderr.

## Dependency

This wrapper depends on the C adapter header and library:

- C header: `Build/adapters/c/include/saikuro.h`
- C library: `saikuro_c` (built from `Build/adapters/c`)

## License

Apache-2.0
