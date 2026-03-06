---
title: "Core Concepts"
description: "How Saikuro works: runtime, adapters, schema, and protocol"
---

Understanding a few core ideas makes everything else click into place. This page covers what's actually happening when you use Saikuro.

## The Four Pieces

Saikuro has four major parts that work together.

### Runtime

A Rust process that sits in the middle of everything. It:

- Loads and validates schemas
- Routes calls to the right provider
- Enforces capabilities
- Manages transports and concurrency

Every invocation passes through the runtime. Adapters don't talk to each other directly.

### Adapters

Thin clients in each language. An adapter's only job is to:

- Serialize your arguments and return values
- Register local functions as providers
- Surface a clean API in the host language

Adapters don't do routing, schema validation, or capability enforcement. That's all in the runtime. Keeping adapters thin is what makes adding new language support tractable.

### Schema

A static description of everything callable in the system:

- Functions (arguments, return type, visibility)
- Types (struct shapes, enums)
- Namespaces (how functions are grouped)
- Capabilities (what token a caller needs to invoke a function)

In development, providers announce their schema automatically. In production, you freeze the schema and generate bindings from it.

### Protocol

MessagePack-encoded envelopes. Every message (call, response, stream frame, error) is a MessagePack object with a defined shape. See the [Protocol Reference](../api/) for the full spec.

## Providers and Callers

**Providers** register functions under a namespace and serve them to callers. Each namespace has exactly one provider in v1. If you have a `math` namespace, one process owns it.

**Callers** connect and invoke functions by their fully-qualified name: `namespace.function`. A caller can call any namespace it has capability tokens for.

Any process can be both a provider and a caller at the same time.

## Namespaces

Functions are addressed as `namespace.function`:

```
math.add
auth.validate_token
images.resize
```

Namespaces keep things organized and make routing unambiguous. The runtime knows which provider owns `math`, so it routes `math.add` to that provider without you doing anything.

## Discovery

### Development Mode

When your provider calls `serve()` in dev mode, it announces its schema to the runtime. The runtime stores it and shares it with callers. This means you can add a new function to your TypeScript provider and a Python caller can start calling it immediately, with no intermediate codegen step.

### Production Mode

Dynamic discovery is disabled. You generate typed bindings from a frozen schema and check them into your repo. This gives you stable, audited interfaces that don't change at runtime.

## Capabilities

Each function can declare required capabilities:

```json
{
  "functions": {
    "delete_user": {
      "capabilities": ["admin.write"]
    }
  }
}
```

Callers present a capability token when they connect. The runtime checks the token against the function's requirements at invocation time. Functions without a capability requirement are callable by anyone.

This is intentionally simple in v1. You don't have to use it at all if you don't need it.

## Visibility

Functions have three visibility levels:

| Level | Who can call it |
| --------- | --------------- |
| `public` | Any caller |
| `internal` | Callers on the same machine |
| `private` | Same process only |

The runtime enforces visibility. Private functions never leave the process boundary.

## The Execution Flow

When a caller invokes `math.add`:

```
caller adapter
  -> serialize args to MessagePack
  -> send Call envelope over transport
  -> runtime receives envelope
  -> runtime validates schema + capabilities
  -> runtime routes to math provider
  -> provider adapter deserializes args
  -> your function runs
  -> provider adapter serializes result
  -> runtime routes response back
  -> caller adapter deserializes result
  -> your code gets the return value
```

The middle of this is invisible. From your perspective: you called a function and got a value back.

## Transports

The transport is how adapters connect to the runtime. Saikuro picks the best transport automatically based on where things are running:

| Situation | Default transport |
| -------------- | ----------------- |
| Same process | In-memory channels |
| Same machine | Unix socket / named pipe |
| Different machines | TCP or WebSocket |

You can override the transport explicitly if you need to. See [Transports](../guide/transports) for details.

## Next Steps

- [Invocation Primitives](../guide/invocations): The six ways to communicate across languages
- [Schema](../guide/schema): How to write and use schemas
- [Transports](../guide/transports): In-memory, sockets, WebSocket
- [Quick Start](./quickstart): If you haven't tried it yet
