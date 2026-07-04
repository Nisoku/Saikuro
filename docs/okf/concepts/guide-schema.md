---
type: concept
title: Schema
description: "Declare your functions, types, capabilities, and namespaces"
source: "https://nisoku.org/Saikuro/guide/schema/"
path: /guide/schema/
updated: 2026-07-04
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-07-04T10:28:43.645Z"
---
---
title: "Schema"
description: "Declare your functions, types, capabilities, and namespaces"
---

The schema is the source of truth for everything callable in a Saikuro system. The runtime uses it to validate calls, enforce capabilities, and route correctly. The codegen tool uses it to emit typed client stubs.

## Schema Structure

Schemas are JSON (or MessagePack). Here is a complete example:

```json
{
  "version": 1,
  "namespaces": {
    "math": {
      "functions": {
        "add": {
          "args": [{ "name": "a", "type": { "kind": "primitive", "type": "i64" }, "optional": false }],
          "returns": { "kind": "primitive", "type": "i64" },
          "visibility": "public",
          "capabilities": [],
          "idempotent": true,
          "doc": "Add two integers."
        }
      }
    }
  },
  "types": {
    "User": {
      "fields": {
        "id": "string",
        "name": "string"
      }
    }
  }
}
```

## Namespaces

Functions live inside namespaces. Each namespace is owned by exactly one provider.

```text
math.add
auth.validate_token
images.resize
```

## Type Descriptors

The `type` field in arguments and return values uses Saikuro's type descriptor system:

| Example                                                   | Meaning                    |
|-----------------------------------------------------------|----------------------------|
| `{ "kind": "primitive", "type": "i32" }`                  | 32-bit signed integer      |
| `{ "kind": "primitive", "type": "string" }`               | UTF-8 string               |
| `{ "kind": "list", "item": { ... } }`                     | List of items              |
| `{ "kind": "map", "key": { ... }, "value": { ... } }`     | Key-value map              |
| `{ "kind": "optional", "inner": { ... } }`                | Nullable value             |
| `{ "kind": "named", "name": "User" }`                     | Reference to a custom type |
| `{ "kind": "stream", "item": { ... } }`                   | Stream of items            |
| `{ "kind": "channel", "send": { ... }, "recv": { ... } }` | Bidirectional channel      |

**Primitive types** `bool`, `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `f32`, `f64`, `string`, `bytes`, `any`, `unit`.

## TypeScript Provider Type Builder

The TypeScript adapter provides a `t` builder for constructing type descriptors at registration:

```typescript
import { SaikuroProvider, t } from "@nisoku/saikuro";

const provider = new SaikuroProvider("math");

provider.register("add", (a: number, b: number) => a + b, {
  args: [
    { name: "a", type: t.i32() },
    { name: "b", type: t.i32() },
  ],
  returns: t.i32(),
  doc: "Add two integers.",
  idempotent: true,
});
```

Available builders:

- `t.bool()`, `t.i32()`, `t.i64()`, `t.f32()`, `t.f64()`, `t.string()`, `t.bytes()`, `t.any()`, `t.unit()`
- `t.list(item)`, `t.map(key, value)`, `t.options(inner)`, `t.named(name)`
- `t.stream(item)`, `t.channel(send, recv)`

## Capabilities

Functions can require capability tokens:

```json
"delete_user": {
  "capabilities": ["admin.write"]
}
```

Callers present their token on connect. If a capability is missing, the call is rejected before it reaches the provider.

In dev mode you can configure the runtime to skip capability enforcement.

## Visibility

Three levels:

| Level      | Who can call                |
|------------|-----------------------------|
| `public`   | Any caller, any machine     |
| `internal` | Callers on the same machine |
| `private`  | Same process only           |

Private functions are never exposed beyond the process boundary.

## Schema Announcement (Dev Mode)

In development, providers announce their schema automatically when they call `serve()`:

```typescript
// Schema is announced automatically
await provider.serve("unix:///tmp/saikuro.sock");
```

The runtime stores the schema and shares it with all connected callers. No codegen step needed during development.

You can also extract a schema statically using your adapter's CLI tool:

```bash
npx saikuro-schema my-namespace provider.ts   # TypeScript
saikuro-schema --namespace my-namespace provider.py  # Python
```

## Schema Announcement (Production)

In production, pass a frozen schema to the runtime:

```bash
saikuro-runtime --schema ./schema.json
```

Dynamic announcement is disabled. Providers that announce a mismatched schema are rejected.

## Next Steps

::: grids
::: grid
::: button "Code Generation" ./codegen.md icon:cpu
:::
::: grid
::: button "Transports" ./transports.md icon:radio
:::
::: grid
::: button "Language Adapters" ../adapters/ icon:code
:::
:::
