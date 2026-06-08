/**
 * Transport abstraction for the TypeScript Saikuro adapter.
 *
 * Implementations:
 *   - NodeUnixTransport  - Unix domain socket (Node.js, native only)
 *   - NodeTcpTransport   - TCP (Node.js, native only)
 *   - WebSocketTransport - WebSocket (Node.js + browser + WASM)
 *   - InMemoryTransport  - In-process (testing)
 *
 * All implement the `Transport` interface.
 */

import { NodeStreamTransport } from "./node-stream";
import { WebSocketTransport } from "./websocket";
import { WasmHostTransport } from "./wasmhost";
import { Transport } from "./types";

export type { Transport } from "./types";
export { InMemoryTransport } from "./memory";
export { WebSocketTransport } from "./websocket";
export { NodeStreamTransport } from "./node-stream";
export { WasmHostTransport } from "./wasmhost";
export { WasmHostConnector, WasmHostListener } from "./wasmhost";

//  Factory

/**
 * Construct the best transport for a given address string.
 *
 * Formats:
 *   - `unix:///path/to/socket`
 *   - `tcp://host:port`
 *   - `ws://host:port/path`  or  `wss://host:port/path`
 *   - `wasm-host://channel-name` (BroadcastChannel for same-origin browser contexts)
 *   - `wasm-host` (uses default channel "saikuro")
 */
const _TRANSPORT_FACTORIES: ReadonlyMap<
  string,
  (address: string) => Transport
> = new Map([
  ["unix://", (addr) => NodeStreamTransport.unix(addr.slice("unix://".length))],
  [
    "tcp://",
    (addr) => {
      const rest = addr.slice("tcp://".length);
      let host: string;
      let portStr: string;
      if (rest.startsWith("[")) {
        const closeBracket = rest.indexOf("]");
        if (closeBracket === -1)
          throw new Error(`Invalid IPv6 address: ${addr}`);
        host = rest.slice(1, closeBracket);
        portStr = rest.slice(closeBracket + 2);
      } else {
        const lastColon = rest.lastIndexOf(":");
        host = rest.slice(0, lastColon);
        portStr = rest.slice(lastColon + 1);
      }
      const port = parseInt(portStr, 10);
      if (isNaN(port)) throw new Error(`Invalid port in address: ${addr}`);
      return NodeStreamTransport.tcp(host, port);
    },
  ],
]);

export function makeTransport(address: string): Transport {
  for (const [prefix, factory] of _TRANSPORT_FACTORIES) {
    if (address.startsWith(prefix)) return factory(address);
  }
  if (address.startsWith("ws://") || address.startsWith("wss://")) {
    return new WebSocketTransport(address);
  }
  if (address === "wasm-host" || address.startsWith("wasm-host://")) {
    let channelName = "saikuro";
    if (address.startsWith("wasm-host://")) {
      const rest = address.slice("wasm-host://".length);
      if (rest.length > 0) {
        channelName = rest;
      }
    }
    return new WasmHostTransport(channelName);
  }
  throw new Error(
    `unsupported transport address: "${address}"\n` +
      "Supported schemes: unix://, tcp://, ws://, wss://, wasm-host://",
  );
}
