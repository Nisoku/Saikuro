---
title: "TypeScript Adapter"
description: "TypeScript and JavaScript adapter for Saikuro"
---

The TypeScript adapter runs in Node.js and, with WebSocket transport, in browsers.

## Install

```bash
npm install @nisoku/saikuro
```

## Client

```typescript
import { Client } from "@nisoku/saikuro";

const client = await Client.connect("tcp://127.0.0.1:7700");
const result = await client.call("math.add", [1, 2]);
console.log(result);
```

## Provider

```typescript
import { Provider } from "@nisoku/saikuro";

const provider = new Provider("math");

provider.register("add", async ([a, b]) => {
  return Number(a) + Number(b);
});

await provider.serve("tcp://127.0.0.1:7700");
```

## Development

```bash
npm ci
npm run build
npm test
npm run typecheck
npm run lint
```

## Next Steps

- [Code Generation](../../guide/codegen): Generate TypeScript stubs from schema
- [TypeScript API Reference](./api-reference): Client and provider method reference
- [TypeScript examples](./examples): TypeScript with Python and Rust integration