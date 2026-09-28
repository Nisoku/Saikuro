/**
 * Math example: one provider and one client over a selectable transport.
 *
 * Build and run:
 *   npm install
 *   npm run build
 *   npm start                              # in-memory (default)
 *   npm start -- --transport tcp           # loopback TCP
 *   npm start -- --transport tcp --addr 127.0.0.1:9000
 */

import { createServer } from "node:net";
import {
  InMemoryTransport,
  NodeStreamTransport,
  SaikuroClient,
  SaikuroError,
  SaikuroProvider,
  type Transport,
} from "@nisoku/saikuro";

/** Which transport the example wires the provider and client over. */
type TransportChoice = "memory" | "tcp";

/** Loopback host used when no `--addr` is given. */
const LOOPBACK = "127.0.0.1";

/** Default listen port. 0 asks the OS for a free port. */
const DEFAULT_PORT = 0;

/** Parsed command line. */
interface Options {
  transport: TransportChoice;
  host: string;
  port: number;
}

/** Read `--transport` / `--addr` from argv, defaulting to in-memory. */
function parseOptions(argv: readonly string[]): Options {
  const options: Options = {
    transport: "memory",
    host: LOOPBACK,
    port: DEFAULT_PORT,
  };

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    switch (arg) {
      case "--transport": {
        const value = argv[++i];
        if (value !== "memory" && value !== "tcp") {
          throw new Error(
            `unknown transport '${String(value)}': expected 'memory' or 'tcp'`,
          );
        }
        options.transport = value;
        break;
      }
      case "--addr": {
        const value = argv[++i];
        const [host, port] = String(value).split(":");
        if (host === undefined || port === undefined) {
          throw new Error(`invalid --addr '${String(value)}': expected HOST:PORT`);
        }
        const parsed = Number.parseInt(port, 10);
        if (!Number.isInteger(parsed) || parsed < 0 || parsed > 65535) {
          throw new Error(`invalid --addr port '${port}'`);
        }
        options.host = host;
        options.port = parsed;
        break;
      }
      case "--help":
      case "-h": {
        console.log(
          "usage: math [--transport memory|tcp] [--addr HOST:PORT]",
        );
        process.exit(0);
        break;
      }
      default:
        throw new Error(`unrecognised argument '${String(arg)}'`);
    }
  }

  return options;
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

/**
 * Wire provider and client directly over a paired in-memory transport.
 */
async function runInMemory(): Promise<void> {
  console.log("transport: in-memory");

  const [providerTransport, clientTransport] = InMemoryTransport.pair();
  await serveOverPair(providerTransport, clientTransport);
}

/**
 * Bind a loopback TCP listener, serve the provider on the accepted connection,
 * and have the client dial it.
 */
async function runTcp(options: Options): Promise<void> {
  const server = createServer();
  await new Promise<void>((resolve) => {
    server.listen(options.port, options.host, resolve);
  });

  const address = server.address();
  if (address === null || typeof address === "string") {
    throw new Error("expected an IP socket address");
  }
  console.log(
    `transport: tcp (provider listening on ${address.address}:${address.port})`,
  );

  const accepted = new Promise<import("node:net").Socket>((resolve) => {
    server.once("connection", (socket) => resolve(socket));
  });

  // The provider adopts the accepted socket; the client dials it.
  const clientTransport = NodeStreamTransport.tcp(
    address.address,
    address.port,
  );
  await clientTransport.connect();

  const socket = await accepted;
  await serveOverPair(
    NodeStreamTransport.fromSocket(socket),
    clientTransport,
  );

  await new Promise<void>((resolve) => server.close(() => resolve()));
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

  await runDemo(client);

  await client.close();
  await servePromise.catch((err: unknown) => {
    console.error("serve failed:", err);
  });
}

async function main(): Promise<void> {
  const options = parseOptions(process.argv.slice(2));
  switch (options.transport) {
    case "memory":
      await runInMemory();
      break;
    case "tcp":
      await runTcp(options);
      break;
  }
  console.log("all examples passed");
}

await main();
