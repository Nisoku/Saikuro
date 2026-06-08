import { getLogger } from "@nisoku/saikuro";
import { startCProvider } from "./wasm/c";
import { startCppProvider } from "./wasm/cpp";
import { startCSharpProvider } from "./wasm/csharp";
import { startPythonProvider } from "./wasm/python";

const log = getLogger("demo.providers");

let started = false;

export async function startProviders(channel: string): Promise<void> {
  if (started) return;
  started = true;

  log.info("connecting all WASM providers in parallel");

  // Each module loads its WASM, creates a Provider, registers handlers,
  // and enters the serve loop, all inside the native language adapter.
  // TypeScript just kicks them off.
  await Promise.all([
    startCProvider(channel),
    startCppProvider(channel),
    startCSharpProvider(channel),
    startPythonProvider(channel),
  ]);

  log.info("all providers started");
}
