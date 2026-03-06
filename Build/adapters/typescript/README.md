# saikuro TypeScript/JavaScript adapter

TypeScript/JavaScript adapter for the [Saikuro](https://github.com/Nisoku/Saikuro)
cross-language IPC fabric. Works in Node.js and (via WebSocket transport) in the
browser.

## Installation

```bash
npm install @nisoku/saikuro
# or
yarn add @nisoku/saikuro
# or
pnpm add @nisoku/saikuro
```

## Usage

### Client

```typescript
import { Client } from "@nisoku/saikuro";

const client = await Client.connect("tcp://127.0.0.1:7700");

const result = await client.call("math.add", [1, 2]);
console.log(result); // 3
```

### Provider

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
npm run build      # compile with tsup
npm test           # run vitest
npm run typecheck  # tsc --noEmit
npm run lint       # eslint
npm run lint:fix   # eslint --fix
```

## License

Apache-2.0
