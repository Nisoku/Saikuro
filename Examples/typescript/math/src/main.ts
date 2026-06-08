/**
 * Math example
 *
 * Wires a math provider and client together over an in-memory transport,
 * then exercises call, cast, batch, and error-handling paths.
 *
 * Build and run:
 *   npm install
 *   npm run build
 *   npm start
 */

import {
  SaikuroProvider,
  SaikuroClient,
  SaikuroError,
  InMemoryTransport,
} from "@nisoku/saikuro";

// provider setup

const provider = new SaikuroProvider("math");

provider.register("add", (a: number, b: number): number => a + b);
provider.register("subtract", (a: number, b: number): number => a - b);
provider.register("multiply", (a: number, b: number): number => a * b);
provider.register("divide", (a: number, b: number): number => {
  if (b === 0) throw new Error("division by zero");
  return a / b;
});

// wire provider + client over in-memory transport
const [providerTransport, clientTransport] = InMemoryTransport.pair();

// Serve in the background, serveOn runs until the transport closes.
const servePromise = provider.serveOn(providerTransport);

const client = await SaikuroClient.openOn(clientTransport);

// call
const sum = await client.call("math.add", [10, 32]);
console.log(`math.add(10, 32) = ${sum}`);
console.assert(sum === 42, `expected 42, got ${sum}`);

const diff = await client.call("math.subtract", [100, 58]);
console.log(`math.subtract(100, 58) = ${diff}`);
console.assert(diff === 42, `expected 42, got ${diff}`);

const product = await client.call("math.multiply", [6, 7]);
console.log(`math.multiply(6, 7) = ${product}`);
console.assert(product === 42, `expected 42, got ${product}`);

const quotient = await client.call("math.divide", [84, 2]);
console.log(`math.divide(84, 2) = ${quotient}`);
console.assert(quotient === 42, `expected 42, got ${quotient}`);

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
} catch (err: any) {
  if (err instanceof SaikuroError) {
    console.log(`divide by zero caught: [${err.code}] ${err.message}`);
  } else {
    throw err;
  }
}

await client.close();
await servePromise.catch((err: any) => {
  console.error("serve failed:", err);
});
console.log("all examples passed");
