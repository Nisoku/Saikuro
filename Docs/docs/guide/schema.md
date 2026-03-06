---
title: "Schema"
description: "Declare your functions, types, capabilities, and namespaces"
---

The schema is Saikuro's source of truth for what exists in the system. The runtime uses it to validate calls, enforce capabilities, and route correctly. Your adapters use it to generate or check bindings.

## Schema Structure

Schemas are JSON. Here's a complete example:

```json
{
  "version": 1,
  "namespaces": {
    "math": {
      "doc": "Basic arithmetic operations.",
      "functions": {
        "add": {
          "args": ["i32", "i32"],
          "returns": "i32",
          "visibility": "public",
          "idempotent": true,
          "doc": "Add two integers."
        },
        "divide": {
          "args": ["f64", "f64"],
          "returns": "f64",
          "visibility": "public",
          "capabilities": ["math.divide"],
          "doc": "Divide two numbers. Returns an error if the divisor is zero."
        }
      }
    },
    "admin": {
      "functions": {
        "reset_counters": {
          "args": [],
          "returns": "void",
          "visibility": "internal",
          "capabilities": ["admin.write"]
        }
      }
    }
  },
  "types": {
    "User": {
      "fields": {
        "id": "string",
        "name": "string",
        "role": "string"
      }
    },
    "PageResult": {
      "fields": {
        "items": "list<User>",
        "total": "i32",
        "page": "i32"
      }
    }
  }
}
```

## Namespaces

Functions live inside namespaces. Each namespace is owned by exactly one provider.

```json
"namespaces": {
  "math": { ... },
  "auth": { ... },
  "images": { ... }
}
```

The namespace name is the first part of every function address: `math.add`, `auth.validate`, `images.resize`.

## Functions

Each function declaration has:

```json
"add": {
  "args": ["i32", "i32"],
  "returns": "i32",
  "visibility": "public",
  "capabilities": ["math.basic"],
  "idempotent": false,
  "doc": "Add two integers and return their sum."
}
```

`args` and `returns` use Saikuro's built-in type names or references to types defined in the `types` section.

`capabilities` is optional. Omit it for functions that any caller can invoke.

`idempotent` is optional (defaults to `false`). Mark a function `true` when calling it twice with the same arguments always produces the same result and has no observable side effects. The runtime exposes this in the schema so callers can make smarter retry and caching decisions, but it does not enforce it.

`doc` is optional. A plain-text description of the function. The codegen tool uses this to emit doc comments in generated client code; it has no effect at runtime.

## Built-in Types

| Type | Description |
| ---- | ----------- |
| `bool` | Boolean |
| `i8`, `i16`, `i32`, `i64` | Signed integers |
| `u8`, `u16`, `u32`, `u64` | Unsigned integers |
| `f32`, `f64` | Floats |
| `string` | UTF-8 string |
| `bytes` | Raw bytes |
| `void` | No return value |
| `list<T>` | Ordered list of T |
| `map<K, V>` | Key-value map |
| `option<T>` | Optional T (may be null) |

For structured data, define your own types in the `types` section and reference them by name.

## Custom Types

```json
"types": {
  "User": {
    "fields": {
      "id": "string",
      "name": "string",
      "created_at": "i64"
    }
  }
}
```

Then reference them in function signatures:

```json
"get_user": {
  "args": ["string"],
  "returns": "User"
}
```

Types can reference other types:

```json
"types": {
  "Address": {
    "fields": {
      "street": "string",
      "city": "string"
    }
  },
  "User": {
    "fields": {
      "id": "string",
      "address": "Address"
    }
  }
}
```

## Visibility

Three levels:

| Level | Who can call it |
| ----- | --------------- |
| `public` | Any caller, any machine |
| `internal` | Callers on the same machine only |
| `private` | Same process only |

The runtime enforces these at the transport layer. `private` functions are never exposed over the network regardless of configuration.

## Capabilities

Capabilities are strings. A function can require zero or more:

```json
"delete_everything": {
  "capabilities": ["admin.write", "nuclear.launch"]
}
```

A caller must present a token that grants all required capabilities. If any capability is missing, the call is rejected before it reaches the provider.

In dev mode you can configure the runtime to skip capability enforcement so you can iterate faster. Don't do that in production.

## Schema Versioning

Schemas have a `version` field:

```json
{
  "version": 1,
  ...
}
```

In v1, the version must be `1`. Future versions will add fields, not remove or rename them.

If you change a function's argument types or return type, that's a breaking change. Increment the schema version and update your callers. The runtime will reject calls that don't match the active schema.

## Dev Mode vs. Production

### Dev Mode (Discovery)

Providers announce their schema when they start:

```typescript
const provider = new Provider({ namespace: 'math', dev: true });

provider.register('add', (a: number, b: number) => a + b);

await provider.serve();
// Schema is announced automatically
```

The runtime accepts and stores the schema. Callers can immediately call `math.add` without knowing about the schema in advance.

### Production Mode

You generate a static schema file, commit it to your repo, and pass it to the runtime at startup:

```bash
saikuro-runtime --schema ./schema.json
```

Dynamic announcement is disabled. Any provider that tries to announce a schema not matching the loaded one gets rejected.

To extract the schema from a TypeScript provider:

```bash
npx saikuro extract --provider provider.ts --out schema.json
```

From Python:

```bash
python -m saikuro extract --provider provider.py --out schema.json
```

## Next Steps

- [Code Generation](./codegen): Generate typed client stubs from a frozen schema
- [Transports](./transports): How the protocol moves between processes
- [Language Adapters](./adapters): Schema usage in TypeScript, Python, C#, and Rust
- [Protocol Reference](../api/): The full wire format including schema envelope
