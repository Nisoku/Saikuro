# TODO: Quality Improvements

> Comprehensive audit of DRY violations, SSOT violations, bugs, and code quality issues.
> Generated from systematic code review across all 6 language adapters and Rust core.

---

## HOW TO READ

- **CRITICAL** = production panic, data loss, or massive duplication
- **HIGH** = significant DRY/SSOT violation, reliability issue, or anti-pattern
- **MEDIUM** = moderate code smell, minor duplication, or inconsistency
- **LOW** = style, naming, tiny duplication

---

## CRITICAL

### C5. Two separate `TypeDescriptor` class hierarchies in C# for the same protocol concept (SSOT)

**Files:**

- `Build/adapters/csharp/Saikuro/src/Provider.cs:51-154`
- `Build/adapters/csharp/Saikuro/src/SchemaExtractor.cs:36-212`
**Issue:** `Provider.cs` has one `TypeDescriptor` hierarchy with `ToWire()` serialization. `SchemaExtractor.cs` has a second hierarchy with `FromType()` factory methods and JSON attributes. Same concept, 14 total classes, two files. Any protocol type change requires editing both.

### C6. Schema announcement dictionary built in 3 separate places in C# alone (SSOT)

**Files:**

- `Build/adapters/csharp/Saikuro/src/Provider.cs:275-325`
- `Build/adapters/csharp/Saikuro/src/SchemaExtractor.cs:639-691`
- `Build/adapters/csharp/Saikuro/src/Envelope.cs:205-213`
**Issue:** Same `{version, namespaces: {ns: {functions: {...}}}, types: {...}}` structure manually constructed with hardcoded string keys in 3 places.

---

## HIGH

### H17. Hand-rolled JSON serialization in C++ schema extractor instead of using a library (SSOT)

**File:** `Build/adapters/cpp/src/schema_extractor.cpp:533-643`
**Issue:** `write_type_obj` and `extract_schema_from_header` manually build JSON with `out << '"' << key << '"' << ':'`. No escaping validation, no structural validation. Rust side uses `serde_json::to_string_pretty` for the exact same purpose.

### H20. Encapsulation violation: public fields on `StreamState` / `ChannelState` bypass accessors

**File:** `Build/crates/saikuro-router/src/stream_state.rs:22-26` and `61-71`
**Issue:** All fields are `pub` even though the structs have public accessor methods (`advance_seq`, `mark_closed`, `is_closed`). External code can bypass safety checks.

### H22. `SaikuroProvider` test code in `provider.ts:369-383` creates an inline Transport implementation

**File:** `Build/adapters/typescript/src/provider.ts:369-383`
**Issue:** Creates a complete `Transport` implementation inline as a plain object just to capture dispatch results. ~15 lines of boilerplate in the middle of `_dispatchBatch`.

### H24. Public fields mutable on `StreamState`/`ChannelState` — bypasses safety protocol

**File:** `Build/crates/saikuro-router/src/stream_state.rs:22-26`
**Issue:** `pub next_seq: AtomicU64`, `pub closed: AtomicBool`, `pub item_tx: mpsc::Sender<ResponseEnvelope>` — external code can write without sequence checking or cleanup.

### H25. `schema_extractor.ts` disables `no-explicit-any` for the entire file

**File:** `Build/adapters/typescript/src/schema_extractor.ts:15`
**Issue:** `/* eslint-disable @typescript-eslint/no-explicit-any */` covers 700+ lines. Pervasive `as any` throughout.

---

## MEDIUM

### M2. TCP/Unix sender/receiver near-duplicates in Rust transport

**Files:**

- `Build/crates/saikuro-transport/src/tcp.rs:80-89` and `98-111`
- `Build/crates/saikuro-transport/src/unix.rs:74-83` and `92-105`
**Issue:** `TransportSender::send`/`close` and `TransportReceiver::recv` are structurally identical. Only log-field names differ.

### M6. `run()` in `connection.rs` is 107 lines, `handle_frame` is 80 lines

**File:** `Build/crates/saikuro-runtime/src/connection.rs:133-240` and `248-328`
**Issue:** Overly long functions. `run` has deeply nested `select!` → match → match → if/else. `handle_frame` does decode, system envelope handling, validation, capability check, and routing.

### M11. Near-identical transport adapter implementations in Rust adapter

**File:** `Build/adapters/rust/src/transport.rs:48-80` (TcpAdapter), `102-132` (UnixAdapter), `150-178` (WsAdapter), `197-228` (WasmHostAdapter)
**Issue:** All implement `send()`, `recv()`, `close()` as identical delegations.

### M12. `c()`, `take_c_string()`, `take_error()` helper functions duplicated across 4 C adapter test files

**Files:** `c_api_protocol.rs:24-39`, `c_api_runtime.rs:24-39`, `c_api_smoke.rs:12-21`, `c_api_validation.rs:14-29`
**Issue:** Identical helper functions for C string conversion duplicated across test files.

### M13. Schema factory builders duplicated across multiple test files

**Files:** `announce_dispatch.rs:31-57`, `cross_language_wire.rs:42-83`, `resource_dispatch.rs:31-57`, `sandbox_dispatch.rs:27-86`
**Issue:** All manually construct `HashMap<String, FunctionSchema>`, `HashMap<String, NamespaceSchema>`, and `Schema { version: 1, namespaces, types }`.

### M15. `in_memory_transport.rs` duplicates `transport_compliance.rs`

**Files:**

- `Build/tests/tests/in_memory_transport.rs`
- `Build/tests/tests/transport_compliance.rs`
**Issue:** 8-9 tests are nearly identical. The compliance suite was designed to be reusable; the in_memory copy should be removed.

### M16. `resource_dispatch.rs:round_trip_via_handler()` duplicates `announce_dispatch.rs:round_trip()`

**Files:**

- `Build/tests/tests/resource_dispatch.rs:101-139`
- `Build/tests/tests/announce_dispatch.rs:72-122`
**Issue:** Identical `ConnectionHandler` construction with all 10 fields, identical `MemoryTransport` pair setup, identical send/drop/run/recv pattern.

### M17. Schema extractor CLI pattern duplicated between Rust and C adapters

**Files:**

- `Build/adapters/rust/src/cli/saikuro_rust_schema.rs`
- `Build/adapters/c/src/cli/saikuro_c_schema.rs`
**Issue:** Identical `primitive()` helper, identical `Args` struct, identical `main()` (`read_to_string` → `extract_schema` → `to_string` → `println!`), identical `#[arg(long, default_value = "default")]` options.

### M18. `CallAsync`/`ResourceAsync`/`BatchAsync` in C# repeat same error-handling pattern 3x

**File:** `Build/adapters/csharp/Saikuro/src/Client.cs:253-370`
**Issue:** All follow same 4-step pattern: create envelope → `SendAndWaitAsync` → check `!resp.Ok` → throw. Three copies of the same logic.

### M19. `seq`/`stream_control` extraction duplicated in `Envelope.FromMsgpackDict` and `ResponseEnvelope.FromMsgpackDict`

**File:** `Build/adapters/csharp/Saikuro/src/Envelope.cs:246-257` vs `323-334`
**Issue:** Identical extraction logic repeated back-to-back.

### M21. C# `ResourceHandle.FromMap` has manual numeric coercion pattern 3x

**File:** `Build/adapters/csharp/Saikuro/src/Envelope.cs:111-135`, `249-257`, `326-334`
**Issue:** `s switch { long l => l, int i => (long)i, ulong u => (long)u, _ => (long?)null }` — same coercion repeated 3 times.

### M22. Schema extractor test path hardcoded twice in CSharp

**File:** `Build/adapters/csharp/Saikuro/tests/SchemaExtractorTests.cs:15-16`, `36-37`
**Issue:** `Path.Combine(repoRoot, "Build", "adapters", "csharp", "tools", "extractor", "extractor.csproj")` appears twice.

### M28. Triple-repeated send/wait/error pattern in Python `call()`, `resource()`, `batch()`

**File:** `Build/adapters/python/saikuro/client.py:115-275`
**Issue:** ~15 identical lines (create future → register → send → wait → cleanup → error check) duplicated across all three methods.

### M29. `_send_frame()` frame-size check duplicated in `WebSocketTransport.send()`

**File:** `Build/adapters/python/saikuro/transport.py:37-43` and `288-291`
**Issue:** Identical `len(data) > _MAX_FRAME_SIZE` check in both places.

### M30. Module-level mutable global state for memory channels in Python

**File:** `Build/adapters/python/saikuro/transport.py:386`
**Issue:** `_memory_channels: dict[str, "InMemoryTransport"] = {}` — global mutable dict means test isolation is not guaranteed.

### M32. Three `Register` overloads with nearly identical bodies in C# provider

**File:** `Build/adapters/csharp/Saikuro/src/Provider.cs:226-268`
**Issue:** Each creates a `HandlerEntry` with a one-line lambda adapter. Could use a single `RegisterCore` method.

### M33. C# `Provider.SchemaDict()` is 51 lines with 4 levels of nesting

**File:** `Build/adapters/csharp/Saikuro/src/Provider.cs:275-325`
**Issue:** Overly long, deeply nested, builds the announcement dictionary manually via string keys.

### M34. Hand-rolled `json_escape` in C++ duplicates Rust's `serde_json`

**File:** `Build/adapters/cpp/src/schema_extractor.cpp:329-364`
**Issue:** Character-by-character JSON string escaping re-implemented. Rust already uses `serde_json` for the same format.

### M36. Regex for C++ function parsing cannot handle nested parens

**File:** `Build/adapters/cpp/src/schema_extractor.cpp:480-481`
**Issue:** `[^)]*` for arguments cannot match nested parentheses. Function-pointer params and default args with parens will break.

### M38. C# `Transport.cs:74` hardcoded 16 MiB max frame size

**File:** `Build/adapters/csharp/Saikuro/src/Transport.cs:74`
**Issue:** `const int MaxFrameSize = 16 * 1024 * 1024` — magic number.

### M40. `SchemaExtractor.cs` uses regex-based XML parsing instead of LINQ to XML

**File:** `Build/adapters/csharp/Saikuro/src/SchemaExtractor.cs:376-426`
**Issue:** Fragile regex `[\s\S]*?` with backtracking instead of `System.Xml.Linq`.

### M41. `Saikuro.cs` has 39 hand-maintained type aliases

**File:** `Build/adapters/csharp/Saikuro/src/Saikuro.cs:55-96`
**Issue:** Every new public type must be manually added. If a type is renamed, this file silently breaks.

### M42. `call()`, `resource()`, `batch()` in TypeScript `client.ts` repeat `as unknown as Record<string, unknown>` cast 5+ times

**File:** `Build/adapters/typescript/src/client.ts:371,509,538,568,585`
**Issue:** `transport.send(envelope as unknown as Record<string, unknown>)`. The `Transport.send()` type should accept `Envelope` directly.

### M45. `provider.ts:532-583` — `_announce` has complex timer/listener management with 3 cleanup paths

**File:** `Build/adapters/typescript/src/provider.ts:532-583`
**Issue:** 52 lines with manual `setTimeout` cancellation, listener registration/removal. High risk of listener leaks on early-return.

### M47. Tuple type handling in schema extractor only uses `items[0]`

**File:** `Build/adapters/typescript/src/schema_extractor.ts:394-407`
**Issue:** `[string, number]` becomes `list(string)`. Should preserve all element types.

### M48. Union types beyond `T | null | undefined` silently return `"any"` in TS schema extractor

**File:** `Build/adapters/typescript/src/schema_extractor.ts:410-430`
**Issue:** Type information silently discarded for complex union/intersection types.

### M49. `buildSchema()` in TS schema_extractor duplicates `provider.ts:schemaObject()`

**File:** `Build/adapters/typescript/src/schema_extractor.ts:646-678` vs `provider.ts:229-273`
**Issue:** Both build the same `{ version, namespaces: { ... } }` structure. Schema construction logic in two files.

### M52. C++ test file has 18 global mutable variables for mock state

**File:** `Build/adapters/cpp/tests/wrapper_behavior.cpp:27-47`
**Issue:** All test mocks use module-level mutable globals. Tests are non-parallelizable and order-dependent.

### M53. Mock C API in test file duplicates the entire `saikuro.h` surface

**File:** `Build/adapters/cpp/tests/wrapper_behavior.cpp:57-254`
**Issue:** Re-implements every function from `saikuro.h` as mock. If the C API changes, this file breaks silently.

### M58. `Stream`/`Channel` response construction uses inline raw dicts in Python provider

**File:** `Build/adapters/python/saikuro/provider.py:222-248`
**Issue:** Stream responses built as raw dicts inline, while call/cast use `_make_ok`/`_make_error` helpers. Inconsistent.

### M59. `Ui` class in build script duplicates `if self._tui` in all 8 methods

**File:** `Build/scripts/saikuro_build.py:50-108`
**Issue:** Every method follows `if self._tui: tui.method(...) else: print(...)`. Should use strategy or decorator.

### M60. `DispatchStreamAsync` in C# sends redundant error + abort frame

**File:** `Build/adapters/csharp/Saikuro/src/Provider.cs:434-449`
**Issue:** On exception, sends both an error response AND an abort frame with `stream_control: "abort"` and `ok: false`. Abort implies failure, so the error response is redundant.

### M61. `Provider.cs:511` fire-and-forget dispatch only captures first inner exception

**File:** `Build/adapters/csharp/Saikuro/src/Provider.cs:511`
**Issue:** `t.Exception?.InnerException?.Message` only captures the first inner exception. `AggregateException` details are lost.

### M62. C# `SchemaExtractorTests.cs:66-68` potential deadlock from `Task.WaitAll` + `WaitForExit`

**File:** `Build/adapters/csharp/Saikuro/tests/SchemaExtractorTests.cs:66-68`
**Issue:** `Task.WaitAll(stdoutTask, stderrTask)` after `proc.WaitForExit()` — if stdout/stderr buffers fill, this deadlocks.

### M63. C# `SchemaExtractorTests.cs:74-80` fragile JSON extraction via `IndexOf('{')` + `LastIndexOf('}')`

**File:** `Build/adapters/csharp/Saikuro/tests/SchemaExtractorTests.cs:74-80`
**Issue:** Nested JSON objects in output would misidentify boundaries.

### M64. C# `Transport.cs:349` `WebSocketTransport.SendAsync` re-checks frame size already checked by `FrameCodec`

**File:** `Build/adapters/csharp/Saikuro/src/Transport.cs:349`
**Issue:** `payload.Length > FrameCodec.MaxFrameSize` — but `FrameCodec.WriteFrameAsync` already validates this.

### M65. C# `Client.cs:383-384` stream/channel handles inserted into dict before `SendAsync` (resource leak on failure)

**File:** `Build/adapters/csharp/Saikuro/src/Client.cs:383-384`, `397-399`
**Issue:** If `SendAsync` throws, the handle remains in the dictionary with no cleanup path.

### M66. C++ `header_compile.cpp` repetitive `static_assert` pattern — 16 assertions that could be a variadic template

**File:** `Build/adapters/cpp/tests/header_compile.cpp:5-21`
**Issue:** Tests `!is_copy_constructible`, `is_move_constructible`, `!is_copy_assignable`, `is_move_assignable` for each of 4 types. Could be a variadic template.

### M69. `_ResultSink` in Python inherits `BaseTransport` to hijack one method (LSP violation)

**File:** `Build/adapters/python/saikuro/provider.py:366-386`
**Issue:** Inherits broad interface to use only `send()`, leaving `connect()`, `close()`, `recv()` as no-op stubs.

### M70. Python `str` + `Enum` mixin anti-pattern in `InvocationType`

**File:** `Build/adapters/python/saikuro/envelope.py:17,30,37`
**Issue:** `class InvocationType(str, enum.Enum)` — breaks type narrowing, leads to subtle dict-key bugs.

### M71. Module-level mutable `_default_provider` in Python

**File:** `Build/adapters/python/saikuro/provider.py:328`
**Issue:** `_default_provider: Optional[SaikuroProvider]` — global state makes the adapter non-reentrant.

### M72. Python `cli.py:36-58` mutates global `sys.path` / `sys.modules`

**File:** `Build/adapters/python/saikuro/cli.py:36-58`
**Issue:** Inserts into `sys.path` then removes in `finally`. If an exception occurs before `finally`, global state is corrupted.

### M74. Multiple Python functions use `Sequence[Any]` or `Any` where specific types should be used

**Files:** `schema.py:100-101`, `stream.py:79`, `provider.py:38`
**Issue:** Bare `Callable[..., Any]`, untyped `send_fn`, etc.

### M76. C# `Envelope.cs:293,296` `Target.LastIndexOf('.') is int i and >= 0` expression repeated back-to-back

**File:** `Build/adapters/csharp/Saikuro/src/Envelope.cs:293,296`
**Issue:** Identical expression used twice. Should be a private property.

### M77. C# `Client.cs:529-530` and `533-534` stream/channel blocks in `DispatchResponse` are near-identical

**File:** `Build/adapters/csharp/Saikuro/src/Client.cs:524-537`
**Issue:** The same `TryRemove` pattern appears in both blocks.

### M78. C# `Logger.cs:104` `"log-{ts}"` ID pattern duplicated with `Client.cs:343`

**File:** `Build/adapters/csharp/Saikuro/src/Logger.cs:104` and `Client.cs:343`
**Issue:** `$"log-{record.Ts}"` pattern in two files.

### M79. C# `Logger.cs` has inconsistent accessibility on convenience overloads

**File:** `Build/adapters/csharp/Saikuro/src/Logger.cs:185-189`
**Issue:** `Error(string msg, string detail)` and `Warn(...)` are `internal` but other overloads are `public`.

### M80. C# `SchemaExtractor.cs:510-513` null-forgiving `!` on nullable reference

**File:** `Build/adapters/csharp/Saikuro/src/SchemaExtractor.cs:510-513`
**Issue:** `assembly.FullName ?? assembly.GetName().Name!` — `!` could throw if both `FullName` and `Name` are null.

---

## LOW

### L3. Compile-error embassy placeholder file

**File:** `Build/crates/saikuro-exec/src/embassy_backend.rs:2`
**Issue:** `compile_error!("embassy-runtime backend is not implemented yet")` — dead stub.

### L12. `transport_compliance.rs:24-27` takes factory `fn() -> (MemoryTransport, MemoryTransport)` — not actually generic

**File:** `Build/tests/tests/transport_compliance.rs:24-27`
**Issue:** Hardcoded to `MemoryTransport`. The word "compliance" is misleading.

### L13. Test naming uses `a_`, `b_`, `c_` prefixes for sort order

**File:** `Build/tests/tests/cross_language_wire.rs`
**Issue:** Fragile naming hack to control test execution order.

### L15. `c_api_validation.rs:14-29` vs `c_api_smoke.rs:12-21` — `take_error()` behaves differently in each

**File:** `Build/adapters/c/tests/c_api_validation.rs:14-29` and `c_api_smoke.rs:12-21`
**Issue:** Same-named helper, but `c_api_smoke` includes null-pointer check while `c_api_validation` does not.

### L17. Saikuro `#[saikuro_test]` proc-macro could eliminate `block_on` boilerplate

**File:** All test files
**Issue:** ~200+ `saikuro_exec::block_on(async { ... })` wrappers across the test suite.

### L18. `as` type assertions in TypeScript `client.ts` that bypass type safety

**File:** `Build/adapters/typescript/src/client.ts:95,96,101,102,107,112,116,119`
**Issue:** `undefined as unknown as T`, `item.result as T`, etc. — discard all type safety.

### L19. Magic number `0` for "no timeout" in TypeScript client

**File:** `Build/adapters/typescript/src/client.ts:268`
**Issue:** `defaultTimeoutMs: options.defaultTimeoutMs ?? 0` — 0 means "no timeout" but is a magic sentinel.

### L20. `_channelSend` constructs raw envelope dict with `target: ""` instead of using factory

**File:** `Build/adapters/typescript/src/client.ts:578-586`
**Issue:** Manual envelope construction duplicates `envelope.ts` factories.

### L21. `Logger._emit` spreads `...extra` that could overwrite base fields

**File:** `Build/adapters/typescript/src/logger.ts:156`
**Issue:** Spreading `...extra` means `{ ts: "malicious" }` could overwrite `ts`, `level`, `name`, `msg`.

### L22. `createLoggingHandler` return type not declared in TypeScript

**File:** `Build/adapters/typescript/src/logging_handler.ts:63`
**Issue:** Missing explicit return type.

### L23. Duplicate list-handling code paths in TypeScript schema extractor

**File:** `Build/adapters/typescript/src/schema_extractor.ts:307-407`
**Issue:** 4 different code paths for handling array-like types (`T[]`, `Array<T>`, `isArrayType`, tuples) with same essential logic.

### L24. `initialize()` is 83 lines of custom `CompilerHost` implementation

**File:** `Build/adapters/typescript/src/schema_extractor.ts:101-184`
**Issue:** Could use `ts.createProgram` directly.

### L25. `70-second hardcoded test timeout

**File:** `Build/adapters/typescript/tests/parity_ts_py.test.ts:243`
**Issue:** `60000` ms magic number.

### L26. `asyncio.sleep(0)` for task yielding in Python tests (flaky anti-pattern)

**File:** `Build/adapters/python/saikuro/client.py:309,552`
**Issue:** Known anti-pattern that creates flaky tests.

### L27. `Transport address parsing logic is convoluted in Python

**File:** `Build/adapters/python/saikuro/transport.py:408-415`
**Issue:** Double-check `address == "memory://"` and `address.startswith("memory://")` is redundant.

### L32. .NET 8 version hardcoded in build script

**File:** `Build/scripts/saikuro_build.py:270`
**Issue:** `_has_dotnet_runtime_8` — magic string `"8."`.

### L33. `CleanXmlText` only collapses double spaces once

**File:** `Build/adapters/csharp/Saikuro/src/SchemaExtractor.cs:428-431`
**Issue:** `"a   b"` (triple space) becomes `"a  b"` (still double). Should use `Regex.Replace(@"\s+", " ")`.

### L34. `Program.cs` in C# tools returns 0 even on failure

**File:** `Build/adapters/csharp/tools/extractor/Program.cs:10-19`
**Issue:** No error handling for schema extraction or JSON serialization failure.

### L35. `SchemaExtractor.cs` returns empty parser silently when file doesn't exist

**File:** `Build/adapters/csharp/Saikuro/src/SchemaExtractor.cs:369-370`
**Issue:** Should throw or log a warning.

### L36. Duplicated doc comments for C API functions in C header

**File:** `Build/adapters/c/include/saikuro.h:106-110` and `120-124`
**Issue:** Exact same 5-line contract block copy-pasted for `channel_next_json` and `stream_next_json`.

### L37. `saikuro_client_close` has no doc comment unlike every other function

**File:** `Build/adapters/c/include/saikuro.h:59`
**Issue:** Missing return-value documentation.

### L38. C++ `saikuro.hpp` `take_owned_c_string` returns by value (extra copy)

**File:** `Build/adapters/cpp/include/saikuro/saikuro.hpp:20-27`
**Issue:** Creates `unique_ptr<char>` then copies into `std::string`. Could avoid copy.

### L40. C++ `saikuro.hpp:294` unused type alias `RawHandler`

**File:** `Build/adapters/cpp/include/saikuro/saikuro.hpp:294`
**Issue:** `using RawHandler = saikuro_provider_handler_fn;` — never referenced.

### L46. Magic string type mappings in C++ schema extractor

**File:** `Build/adapters/cpp/src/schema_extractor.cpp:402-416`
**Issue:** `"string"`, `"bool"`, `"f64"`, `"unit"`, etc. should be constants or an enum.

### L48. C++ raw string literal handling doesn't support prefix variants

**File:** `Build/adapters/cpp/src/schema_extractor.cpp:302-316`
**Issue:** `R"` detection doesn't handle `LR"`, `uR"`, `UR"`, `u8R"`.

### L49. `ChannelSendAsync` builds dictionary with hardcoded string keys

**File:** `Build/adapters/csharp/Saikuro/src/Client.cs:455-466`
**Issue:** `"version"`, `"type"`, `"id"`, `"target"`, `"args"` — same wire keys repeated from other methods.

### L50. C# `DispatchResponse` inserts handles before `SendAsync` — resource leak if `SendAsync` throws

**File:** `Build/adapters/csharp/Saikuro/src/Client.cs:397-399`
**Issue:** Handle remains in `_openStreams`/`_openChannels` dict with no cleanup path.

---

## CROSS-CUTTING: Adapter-Wide Duplication

Every language adapter independently re-implements the same wire protocol logic:

| Pattern               | Rust Adapter          | TypeScript                     | Python                         | CSharp                 |
|-----------------------|-----------------------|--------------------------------|--------------------------------|------------------------|
| Error subclass tree   | rust error.rs         | error.ts (15 classes)          | error.py (14 classes)          | Errors.cs (14 classes) |
| Envelope factories    | N/A (uses core crate) | envelope.ts                    | envelope.py                    | Envelope.cs            |
| Send-and-wait pattern | client.rs             | client.ts                      | client.py                      | Client.cs              |
| Log envelope building | provider.rs           | client.ts + logging_handler.ts | client.py + logging_handler.py | Client.cs + Logger.cs  |
| Transport factory     | transport.rs          | transport.ts                   | transport.py                   | Transport.cs           |
| Schema announcement   | schema.rs             | provider.ts                    | provider.py                    | Provider.cs            |

Every protocol change requires updating **4 independent implementations**. A shared code generation step or shared protocol definition would eliminate this.
