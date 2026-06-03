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

  log.info("starting runtime WASM");
  await startRuntimeWasm(CHANNEL);

  log.info("starting Rust provider (background)");
  startRustProvider(CHANNEL);

  log.info("starting C/C++/C#/Python providers");
  await startProviders(CHANNEL);

  // Providers connect via BroadcastChannel and announce schemas.
  // The Rust provider runs in the background (fire-and-forget WASM), so
  // give it enough time to connect and announce before the client opens.
  await new Promise((r) => setTimeout(r, 250));

  log.info("connecting client via WasmHostConnector");
  const connector = new WasmHostConnector(CHANNEL);
  const transport = await connector.connect();
  log.info("transport connected, opening client");
  const client = await SaikuroClient.openOn(transport);
  log.info("bootRuntime complete, ready");
  return { client };
}
