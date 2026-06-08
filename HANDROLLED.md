# Bad Code Tracker (basically)

## Legend

| Value       | Meaning         |
|-------------|-----------------|
| **partial** | Partially Fixed |
| **no**      | Need to Fix     |

---

## Rust

| What                                                                                 | Where                                        | Replace with                            | Status |
|--------------------------------------------------------------------------------------|----------------------------------------------|-----------------------------------------|--------|
| Short ID generation - `uuid_short()` with `SystemTime` + `AtomicU64`                 | `saikuro-runtime/src/main.rs:321`            | `ulid` or `uuid`                        | **no** |
| Short ID generation - JS `crypto.getRandomValues` byte loop                          | `saikuro-transport/src/wasm_host.rs:53`      | `uuid::Uuid::new_v4()`                  | **no** |
| C/C++ string literal escaping - manual match on special chars                        | `saikuro-codegen/src/cpp.rs:261`, `c.rs:156` | `one_escape` crate                      | **no** |
| URL/address parsing - `strip_prefix` chain + manual host:port splitting              | `adapters/rust/src/transport.rs:216-287`     | `url` crate                             | **no** |
| JSON manual deser - walking `serde_json::Value` variants                             | `adapters/c/src/lib.rs:61-116`               | `#[derive(Deserialize)]` structs        | **no** |
| Channel wrapper - handrolled Sender/Receiver/JoinHandle types around `futures::mpsc` | `saikuro-exec/src/wasm_backend.rs:26-200+`   | Direct futures API or tokio re-export   | **no** |
| LogRecord `ts: String` - raw ISO string instead of typed `DateTime`                  | `saikuro-core/src/log.rs:71`                 | `chrono::DateTime<Utc>` (already a dep) | **no** |

---

## Python

| What                                                           | Where                                 | Replace with                         | Status |
|----------------------------------------------------------------|---------------------------------------|--------------------------------------|--------|
| URI/address parsing - `startswith` chain + IPv6 bracket hack   | `saikuro/transport/__init__.py:60-96` | `urllib.parse` / `yarl`              | **no** |
| HTTP download - shelling out to `curl`                         | `shared/dotnet.py:32`                 | `urllib.request` / `httpx`           | **no** |
| Manual `to_dict()` - dataclasses with handrolled dict builders | `saikuro/envelope.py:61,91,232`       | `dataclasses.asdict()`               | **no** |
| Version parsing - grepping human CLI output                    | `shared/dotnet.py:40`                 | `dotnet --version` + `Version.Parse` | **no** |

---

## TypeScript

| What                                                           | Where                                                    | Replace with                     | Status |
|----------------------------------------------------------------|----------------------------------------------------------|----------------------------------|--------|
| CLI arg parsing - `process.argv` manual dispatch               | `Demo/dev.mjs:5-8,202-230`, `cli/saikuro-schema.ts:4-11` | `commander` or `yargs`           | **no** |
| ANSI colors - handrolled `\x1b[31m` escapes                    | `Demo/dev.mjs:18-34`                                     | `chalk` or `picocolors`          | **no** |
| Logger - entire custom logging framework (~150 lines)          | `src/logger.ts:1-149`                                    | `pino` or `winston`              | **no** |
| Event emitter - pub/sub with `Set` of handlers                 | `transport/base.ts:10-34` (+ 4 subclasses)               | `mitt` or `EventEmitter`         | **no** |
| Async queue - manual `_buffer` + `_waiters` arrays             | `src/client.ts:57-108`                                   | `p-queue`                        | **no** |
| Timeout/retry - manual `setTimeout`/`clearTimeout`             | `src/client.ts:512-542`, `src/provider.ts:727-751`       | `p-retry` / `async-retry`        | **no** |
| Debounce - manual `debounceTimers`                             | `Demo/dev.mjs:92-103`                                    | `lodash.debounce`                | **no** |
| ID generation - `crypto.getRandomValues` byte loop             | `src/envelope.ts:293-313`                                | `uuid` or `nanoid`               | **no** |
| File watcher - handrolled `fs.watch` recursive                 | `Demo/dev.mjs:121-199`                                   | `chokidar`                       | **no** |
| Subprocess runner - manual `spawn` Promise wrapper             | `Demo/dev.mjs:36-51`                                     | `execa`                          | **no** |
| Address parsing - manual IPv6 bracket detection                | `src/transport/index.ts:37-64`                           | `url-parse` or `URL`             | **no** |
| Function param parsing - regex state machine                   | `src/provider.ts:259-348`                                | `ts-morph`                       | **no** |
| Schema extractor - manual TS compiler host (500+ lines)        | `src/schema_extractor.ts:76-617`                         | `ts-morph`                       | **no** |
| Deep comparison - recursive structural canonicalizer           | `tests/canonicalize.ts:1-125`                            | `lodash.isEqual`                 | **no** |
| Frame encoding - manual `Buffer.allocUnsafe` + `writeUInt32BE` | `transport/framing.ts:5-9`                               | built-in `Int32Array`/`DataView` | **no** |

---

## C++

| What                                                                             | Where                          | Replace with                        | Status |
|----------------------------------------------------------------------------------|--------------------------------|-------------------------------------|--------|
| C++ function parser - 280-line state machine with comment removal, nesting, args | `schema_extractor.cpp:52-533`  | `libclang`                          | **no** |
| CLI parsing - `argc`/`argv` if/else chain                                        | `saikuro_cpp_schema.cpp:17-54` | `CLI11`                             | **no** |
| Type mapping - regex-based C++ type→schema                                       | `schema_extractor.cpp:297-349` | `libclang` `clang_getCanonicalType` | **no** |
| trim/split - reimplemented utilities                                             | `schema_extractor.cpp:28-50`   | `Abseil` / `Boost`                  | **no** |
| `strncmp` - handrolled loop                                                      | `insight_cpp.cpp:13-19`        | `<cstring>`                         | **no** |

---

## CSharp

| What                                                                       | Where                           | Replace with                             | Status |
|----------------------------------------------------------------------------|---------------------------------|------------------------------------------|--------|
| Address parsing - manual `string.Split` + `LastIndexOf(':')` (breaks IPv6) | `TransportFactory.cs:42-74`     | `System.Uri`                             | **no** |
| Dict field extraction - manual `TryGetValue` + type checks                 | `Envelope.cs:110-267`           | `MessagePackSerializer.Deserialize<T>()` | **no** |
| CLI args - `args.Length > 0 ? args[0] : "default"`                         | `tools/extractor/Program.cs:14` | `System.CommandLine`                     | **no** |
