import { SaikuroClient, WasmHostConnector } from "@nisoku/saikuro";
import { startRuntimeWasm } from "./wasm/runtime";
import { startRustProvider } from "./wasm/rust";
import { startProviders } from "./providers";

const CHANNEL = "saikuro-insight-lab";
let bootPromise: Promise<RuntimeContext> | null = null;

export type RuntimeContext = {
  client: SaikuroClient;
};

export async function ensureRuntime(): Promise<RuntimeContext> {
  if (!bootPromise) {
    bootPromise = bootRuntime();
  }
  return bootPromise;
}

async function bootRuntime(): Promise<RuntimeContext> {
  await startRuntimeWasm(CHANNEL);
  await startRustProvider(CHANNEL);
  await startProviders(CHANNEL);

  const connector = new WasmHostConnector(CHANNEL);
  const transport = await connector.connect();
  const client = await SaikuroClient.openOn(transport);
  return { client };
}
