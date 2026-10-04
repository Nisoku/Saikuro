/**
 * Math example: one provider and one client over a selectable transport.
 *
 * Build and run:
 *   npm install
 *   npm run build
 *   npm start                              # in-memory (default)
 *   npm start -- --transport tcp           # loopback TCP
 *   npm start -- --transport tcp --addr 127.0.0.1:9000
 *   npm start -- --transport unix          # loopback Unix socket
 *   npm start -- --transport unix --addr /tmp/math.sock
 *   npm start -- --transport ws            # loopback WebSocket
 *   npm start -- --transport ws --addr 127.0.0.1:9000
 *
 * The socket modes host a provider and dial it in the same process. Use
 * `--serve-only` to listen without a local client, or `--client-only` to dial
 * an existing provider without listening.
 */

import { once } from "node:events";
import { existsSync, unlinkSync } from "node:fs";
import { createServer, type Server as NetServer, type Socket } from "node:net";
import {
  InMemoryTransport,
  NodeStreamTransport,
  SaikuroClient,
  SaikuroError,
  SaikuroProvider,
  WebSocketListener,
  type Transport,
} from "@nisoku/saikuro";

/** Which transport the example wires the provider and client over. */
type TransportChoice = "memory" | "tcp" | "unix" | "ws";
type Mode = "both" | "serve-only" | "client-only";

/** Loopback host used when no `--addr` is given. */
const LOOPBACK = "127.0.0.1";

/** Default listen port. 0 asks the OS for a free port. */
const DEFAULT_PORT = 0;

/** Default Unix socket path */
const DEFAULT_UNIX_PATH = "/tmp/saikuro-math.sock";

/** Parsed command line. */
interface Options {
  transport: TransportChoice;
  mode: Mode;
  addr: string;
}

/** The default `--addr` for a transport. */
function defaultAddr(transport: TransportChoice): string {
  switch (transport) {
    case "memory":
      return "";
    case "tcp":
    case "ws":
      return `${LOOPBACK}:${DEFAULT_PORT}`;
    case "unix":
      return DEFAULT_UNIX_PATH;
  }
}

/** Read the command line, defaulting to in-memory over a per-transport address. */
function parseOptions(argv: readonly string[]): Options {
  let transport: TransportChoice = "memory";
  let mode: Mode = "both";
  let addr: string | undefined;

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    switch (arg) {
      case "--transport": {
        const value = argv[++i];
        if (value !== "memory" && value !== "tcp" && value !== "unix" && value !== "ws") {
          throw new Error(
            `unknown transport '${String(value)}': expected 'memory', 'tcp', 'unix', or 'ws'`,
          );
        }
        transport = value;
        break;
      }
      case "--addr": {
        const value = argv[++i];
        if (value === undefined || value.length === 0) {
          throw new Error("--addr needs a value");
        }
        addr = value;
        break;
      }
      case "--serve-only": {
        if (mode === "client-only") {
          throw new Error("--serve-only and --client-only are mutually exclusive");
        }
        mode = "serve-only";
        break;
      }
      case "--client-only": {
        if (mode === "serve-only") {
          throw new Error("--serve-only and --client-only are mutually exclusive");
        }
        mode = "client-only";
        break;
      }
      case "--help":
      case "-h": {
        console.log(
          "usage: math [--transport memory|tcp|unix|ws] [--addr HOST:PORT|PATH] " +
            "[--serve-only|--client-only]",
        );
        process.exit(0);
        break;
      }
      default:
        throw new Error(`unrecognised argument '${String(arg)}'`);
    }
  }

  if (mode !== "both" && transport === "memory") {
    throw new Error("--serve-only/--client-only need a socket transport");
  }

  return { transport, mode, addr: addr ?? defaultAddr(transport) };
}

/**
 * The shared schema: every handler is registered here, for every transport.
 *
 * Handlers receive the invocation arguments as positional parameters, so they
 * are declared as `(...args: unknown[])` and narrow their own operands.
 */
function mathProvider(): SaikuroProvider {
  const provider = new SaikuroProvider("math");

  provider.register("add", (...args) => {
    const [a, b] = operands(args);
    return a + b;
  });
  provider.register("subtract", (...args) => {
    const [a, b] = operands(args);
    return a - b;
  });
  provider.register("multiply", (...args) => {
    const [a, b] = operands(args);
    return a * b;
  });
  provider.register("divide", (...args) => {
    const [a, b] = operands(args);
    if (b === 0) throw new Error("division by zero");
    return a / b;
  });

  return provider;
}

/** Read the two operands a math handler expects, defaulting to zero. */
function operands(args: readonly unknown[]): [number, number] {
  const a = typeof args[0] === "number" ? args[0] : 0;
  const b = typeof args[1] === "number" ? args[1] : 0;
  return [a, b];
}

/**
 * The shared client demo. Identical for every transport.
 */
async function runDemo(client: SaikuroClient): Promise<void> {
  // call

  const sum = await client.call("math.add", [10, 32]);
  console.log(`math.add(10, 32) = ${sum}`);
  console.assert(sum === 42, `expected 42, got ${String(sum)}`);

  const diff = await client.call("math.subtract", [100, 58]);
  console.log(`math.subtract(100, 58) = ${diff}`);
  console.assert(diff === 42, `expected 42, got ${String(diff)}`);

  const product = await client.call("math.multiply", [6, 7]);
  console.log(`math.multiply(6, 7) = ${product}`);
  console.assert(product === 42, `expected 42, got ${String(product)}`);

  const quotient = await client.call("math.divide", [84, 2]);
  console.log(`math.divide(84, 2) = ${quotient}`);
  console.assert(quotient === 42, `expected 42, got ${String(quotient)}`);

  // cast (fire-and-forget)

  await client.cast("math.add", [1, 1]);
  console.log("cast sent (no response expected)");

  // batch

  const [batchSum, batchProduct] = await client.batch([
    { target: "math.add", args: [1, 2] },
    { target: "math.multiply", args: [3, 4] },
  ]);
  console.log(`batch [add(1,2), multiply(3,4)] = [${batchSum}, ${batchProduct}]`);

  // error handling

  try {
    await client.call("math.divide", [1, 0]);
  } catch (err) {
    if (err instanceof SaikuroError) {
      console.log(`divide by zero caught: [${err.code}] ${err.message}`);
    } else {
      throw err;
    }
  }
}

// Transports

/** A bound listener/closable the example can shut down. */
interface Closable {
  close(): unknown;
}

/** Wire provider and client directly over a paired in-memory transport. */
async function runInMemory(): Promise<void> {
  console.log("transport: in-memory");

  const [providerTransport, clientTransport] = InMemoryTransport.pair();
  await serveOverPair(providerTransport, clientTransport);
}

/** Serve the provider until the connection closes, reporting failures. */
async function serveProvider(transport: Transport): Promise<void> {
  try {
    await mathProvider().serveOn(transport);
  } catch (err) {
    console.error("provider: connection failed:", err);
  }
}

/** Parse `HOST:PORT`, naming the transport in any error. */
function parseHostPort(value: string, transport: string): { host: string; port: number } {
  const [host, portText] = value.split(":");
  if (host === undefined || portText === undefined) {
    throw new Error(`invalid --addr '${value}' for ${transport}: expected HOST:PORT`);
  }
  const port = Number.parseInt(portText, 10);
  if (!Number.isInteger(port) || port < 0 || port > 65535) {
    throw new Error(`invalid --addr port '${portText}'`);
  }
  return { host, port };
}

/** Resolve a `net` listen call. */
function listenServer(server: NetServer, target: { host: string; port: number } | { path: string }): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    const onError = (err: Error) => reject(err);
    server.once("error", onError);
    const done = () => {
      server.removeListener("error", onError);
      resolve();
    };
    if ("path" in target) {
      server.listen(target.path, done);
    } else {
      server.listen(target.port, target.host, done);
    }
  });
}

/** Wait for the next inbound connection on a `net` server. */
async function acceptConnection(server: NetServer): Promise<Socket> {
  const [socket] = (await once(server, "connection")) as [Socket];
  return socket;
}

/** Bind a TCP or Unix `net` listener handling the selected mode. */
async function runNodeStream(
  options: Options,
  target: { host: string; port: number } | { path: string },
): Promise<void> {
  if ("path" in target && existsSync(target.path)) {
    // A stale socket file from a previous run would fail the bind.
    unlinkSync(target.path);
  }

  const server = createServer();
  await listenServer(server, target);

  let clientAddress: string;
  let display: string;
  if ("path" in target) {
    clientAddress = `unix://${target.path}`;
    display = target.path;
  } else {
    const address = server.address();
    if (address === null || typeof address === "string") {
      throw new Error("expected an IP socket address");
    }
    clientAddress = `tcp://${address.address}:${address.port}`;
    display = `${address.address}:${address.port}`;
  }
  console.log(`transport: ${options.transport} (provider listening on ${display})`);

  if (options.mode === "serve-only") {
    for (;;) {
      const socket = await acceptConnection(server);
      await serveProvider(NodeStreamTransport.fromSocket(socket));
    }
  }

  // Register the accept before dialling so the connection cannot race it.
  const accepting = acceptConnection(server).then((socket) =>
    serveProvider(NodeStreamTransport.fromSocket(socket)),
  );
  await runClient(clientAddress, accepting, server);
}

/** Bind a WebSocket listener handling the selected mode. */
async function runWebSocket(options: Options): Promise<void> {
  const { host, port } = parseHostPort(options.addr, "ws");
  const listener = await WebSocketListener.bind({ host, port });
  console.log(`transport: ws (provider listening on ${host}:${listener.port})`);

  const clientAddress =
    options.addr.startsWith("ws://") || options.addr.startsWith("wss://")
      ? options.addr
      : `ws://${host}:${listener.port}`;

  if (options.mode === "serve-only") {
    for (;;) {
      const transport = await listener.accept();
      await serveProvider(transport);
    }
  }

  const accepting = listener.accept().then((transport) => serveProvider(transport));
  await runClient(clientAddress, accepting, listener);
}

/** Dial an existing provider and drive the shared demo. */
async function runClientOnly(options: Options): Promise<void> {
  let clientAddress: string;
  switch (options.transport) {
    case "memory":
      clientAddress = "memory";
      break;
    case "tcp":
      clientAddress = `tcp://${options.addr}`;
      break;
    case "unix":
      clientAddress = `unix://${options.addr}`;
      break;
    case "ws":
      clientAddress =
        options.addr.startsWith("ws://") || options.addr.startsWith("wss://")
          ? options.addr
          : `ws://${options.addr}`;
      break;
  }
  console.log(`transport: ${options.transport} (client dialling ${clientAddress})`);
  const client = await SaikuroClient.connect(clientAddress);
  try {
    await runDemo(client);
  } finally {
    await client.close();
  }
}

/**
 * Dial `clientAddress`, run the shared demo, then tear down the client, the
 * provider accept task, and the listener.
 */
async function runClient(
  clientAddress: string,
  accepting: Promise<void>,
  listener: Closable,
): Promise<void> {
  const client = await SaikuroClient.connect(clientAddress);
  try {
    await runDemo(client);
  } finally {
    await client.close();
    await accepting.catch((err: unknown) => console.error("serve failed:", err));
    await listener.close();
  }
}

/**
 * Serve the shared provider on `providerTransport`, drive the shared demo from
 * `clientTransport`, then shut both down.
 */
async function serveOverPair(
  providerTransport: Transport,
  clientTransport: Transport,
): Promise<void> {
  const servePromise = mathProvider().serveOn(providerTransport);
  const client = await SaikuroClient.openOn(clientTransport);

  try {
    await runDemo(client);
  } finally {
    // Tear both sides down even if the demo threw.
    await client.close();
    await servePromise.catch((err: unknown) => {
      console.error("serve failed:", err);
    });
  }
}

async function main(): Promise<void> {
  const options = parseOptions(process.argv.slice(2));

  if (options.transport === "memory") {
    await runInMemory();
  } else if (options.mode === "client-only") {
    await runClientOnly(options);
  } else {
    switch (options.transport) {
      case "tcp":
        await runNodeStream(options, parseHostPort(options.addr, "tcp"));
        break;
      case "unix":
        await runNodeStream(options, { path: options.addr });
        break;
      case "ws":
        await runWebSocket(options);
        break;
    }
  }
  console.log("all examples passed");
}

await main();
