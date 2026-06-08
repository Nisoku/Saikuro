# Saikuro TypeScript/JavaScript adapter

TypeScript/JavaScript adapter for the [Saikuro](https://github.com/Nisoku/Saikuro)
cross-language IPC fabric. Works in Node.js and in the browser through both
WebSocket and `wasm-host://` transports.

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

### Browser transports

The adapter exports both the raw BroadcastChannel transport classes and the
WasmHost aliases used by browser-side demos and wasm runtimes:

```typescript
import {
  WasmHostTransport,
  WasmHostConnector,
  WasmHostListener,
} from "@nisoku/saikuro";

const transport = new WasmHostTransport("saikuro-demo");
await transport.connect();
```

Use `wasm-host` or `wasm-host://channel-name` when you want same-origin
browser contexts, workers, or wasm guests to talk over BroadcastChannel rather
than a network socket.

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
