import { getLogger } from "@nisoku/saikuro";

const log = getLogger("demo.wasm.runtime");

let runtimePromise: Promise<void> | null = null;

export async function startRuntimeWasm(channel: string): Promise<void> {
  if (!runtimePromise) {
    runtimePromise = (async () => {
      log.info("loading runtime WASM", { channel });
      const mod =
        await import("../../../public/wasm/runtime/saikuro_web_runtime.js");
      log.info("runtime WASM module loaded, initializing");
      await mod.default();
      log.info("runtime WASM initialized, starting runtime", { channel });
      await mod.start_runtime(channel);
      log.info("runtime started", { channel });
    })();
  }
  return runtimePromise;
}
