import { getLogger, SaikuroClient, WasmHostConnector } from "@nisoku/saikuro";
import { startRuntimeWasm } from "./wasm/runtime";
import { startRustProvider } from "./wasm/rust";
import { startProviders } from "./providers";

const log = getLogger("demo.runtime");

const CHANNEL = "saikuro-insight-lab";
let bootPromise: Promise<RuntimeContext> | null = null;

export type RuntimeContext = {
  client: SaikuroClient;
};

export async function ensureRuntime(): Promise<RuntimeContext> {
  if (!bootPromise) {
    bootPromise = bootRuntime().catch((err) => {
      log.error("bootRuntime failed, resetting for retry", { err });
      bootPromise = null;
      throw err;
    });
  }
  return bootPromise;
}

async function bootRuntime(): Promise<RuntimeContext> {
  log.info("bootRuntime start", { channel: CHANNEL });

  // Start the Rust-based Saikuro runtime WASM (accept loop on BroadcastChannel)
  await startRuntimeWasm(CHANNEL);

  // Fire all providers in background (each loads its own WASM module and
  //    registers itself with the Runtime via wasm-host transport)
  startRustProvider(CHANNEL);
  await startProviders(CHANNEL);

  // Wait for providers to connect and announce
  await new Promise((r) => setTimeout(r, 500));

  // Create a client that talks to the runtime over the same channel
  log.info("connecting client via WasmHostConnector");
  const connector = new WasmHostConnector(CHANNEL);
  const transport = await connector.connect();
  log.info("transport connected, opening client");
  const client = await SaikuroClient.openOn(transport);
  log.info("bootRuntime complete, ready");
  return { client };
}
