# TODO: Quality Improvements

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

### H22. `SaikuroProvider` test code in `provider.ts:369-383` creates an inline Transport implementation

**File:** `Build/adapters/typescript/src/provider.ts:369-383`
**Issue:** Creates a complete `Transport` implementation inline as a plain object just to capture dispatch results. ~15 lines of boilerplate in the middle of `_dispatchBatch`.

### H25. `schema_extractor.ts` disables `no-explicit-any` for the entire file

**File:** `Build/adapters/typescript/src/schema_extractor.ts:15`
**Issue:** `/* eslint-disable @typescript-eslint/no-explicit-any */` covers 700+ lines. Pervasive `as any` throughout.

---

## MEDIUM

### M11. Near-identical transport adapter implementations in Rust adapter

**File:** `Build/adapters/rust/src/transport.rs:48-80` (TcpAdapter), `102-132` (UnixAdapter), `150-178` (WsAdapter), `197-228` (WasmHostAdapter)
**Issue:** All implement `send()`, `recv()`, `close()` as identical delegations.

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

### M28. Triple-repeated send/wait/error pattern in Python `call()`, `resource()`, `batch()`

**File:** `Build/adapters/python/saikuro/client.py:115-275`
**Issue:** ~15 identical lines (create future → register → send → wait → cleanup → error check) duplicated across all three methods.

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

### M45. `provider.ts:532-583` — `_announce` has complex timer/listener management with 3 cleanup paths

**File:** `Build/adapters/typescript/src/provider.ts:532-583`
**Issue:** 52 lines with manual `setTimeout` cancellation, listener registration/removal. High risk of listener leaks on early-return.

### M52. C++ test file has 18 global mutable variables for mock state

**File:** `Build/adapters/cpp/tests/wrapper_behavior.cpp:27-47`
**Issue:** All test mocks use module-level mutable globals. Tests are non-parallelizable and order-dependent.

### M53. Mock C API in test file duplicates the entire `saikuro.h` surface

**File:** `Build/adapters/cpp/tests/wrapper_behavior.cpp:57-254`
**Issue:** Re-implements every function from `saikuro.h` as mock. If the C API changes, this file breaks silently.

### M69. `_ResultSink` in Python inherits `BaseTransport` to hijack one method (LSP violation)

**File:** `Build/adapters/python/saikuro/provider.py:366-386`
**Issue:** Inherits broad interface to use only `send()`, leaving `connect()`, `close()`, `recv()` as no-op stubs.

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
