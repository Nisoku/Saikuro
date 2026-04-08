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

## Dependency

This wrapper depends on the C adapter header and library:

- C header: `Build/adapters/c/include/saikuro.h`
- C library: `saikuro_c` (built from `Build/adapters/c`)

## License

Apache-2.0
