---
type: api
title: "CLI Reference"
description: "Saikuro runtime command-line interface"
source: "https://nisoku.org/Saikuro/api/cli/"
path: /api/cli/
updated: 2026-07-15
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-07-15T17:26:24.471Z"
---
---
title: "CLI Reference"
description: "Saikuro runtime command-line interface"
---

## `saikuro-runtime`

The Saikuro runtime server. Routes invocations between adapters over TCP, WebSocket, and Unix domain sockets.

```text
saikuro-runtime [OPTIONS]
```

### Options

| Flag                  | Default       | Description                                                  |
|-----------------------|---------------|--------------------------------------------------------------|
| `--schema <PATH>`     | -             | Load a frozen schema JSON at startup                         |
| `--tcp-port <PORT>`   | `7700`        | Port for TCP connections. `0` = OS-assigned                  |
| `--ws-port <PORT>`    | `7701`        | Port for WebSocket connections. `0` = OS-assigned            |
| `--unix <PATH>`       | -             | Path for Unix domain socket listener                         |
| `--bind <ADDR>`       | `127.0.0.1`   | Bind address for TCP/WebSocket                               |
| `--mode <MODE>`       | `development` | Runtime mode: `development` or `production`                  |
| `--log-level <LEVEL>` | `info`        | Minimum log level: `error`, `warn`, `info`, `debug`, `trace` |
| `--json-logs`         | -             | Emit logs as newline-delimited JSON                          |
| `--no-tcp`            | -             | Disable the TCP listener                                     |
| `--no-ws`             | -             | Disable the WebSocket listener                               |

### Examples

Start the runtime in production mode with a frozen schema:

```bash
saikuro-runtime \
  --schema ./schema.json \
  --mode production \
  --tcp-port 7700 \
  --unix /tmp/saikuro.sock \
  --bind 0.0.0.0
```

Start with WebSocket only:

```bash
saikuro-runtime --no-tcp --ws-port 8080
```

Start with JSON logs for log aggregation:

```bash
saikuro-runtime --json-logs --log-level debug
```

## `saikuro-codegen`

Generate typed client stubs from a schema file.

```bash
saikuro-codegen --schema <PATH> --lang <LANG> --out <DIR>
```

| Flag              | Description                                                           |
|-------------------|-----------------------------------------------------------------------|
| `--schema <PATH>` | Schema JSON file to generate from                                     |
| `--lang <LANG>`   | Target language: `typescript`, `python`, `csharp`, `rust`, `c`, `cpp` |
| `--out <DIR>`     | Output directory                                                      |

## Schema Extraction

Each language adapter provides a CLI to extract a schema from source files:

**TypeScript:**

```bash
npx saikuro-schema <namespace> <file1> [file2...]
```

**Python:**

```bash
saikuro-schema --namespace <NAME> [--output <FILE>] <FILE>
```

## Environment Variables

| Variable      | Default | Description                       |
|---------------|---------|-----------------------------------|
| `SAIKURO_LOG` | `info`  | Minimum log level for the runtime |
