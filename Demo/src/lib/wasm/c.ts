import { getLogger } from "@nisoku/saikuro";

const log = getLogger("demo.wasm.c");

let bootPromise: Promise<void> | null = null;

export async function startCProvider(channel: string): Promise<void> {
  if (!bootPromise) {
    bootPromise = (async () => {
      log.info("loading C WASM", { channel });
      const mod = await import(
        "../../../public/wasm/c/saikuro_c_insight.js"
      );
      log.info("C WASM module loaded, initializing");
      await mod.default();
      log.info("C WASM initialized, starting provider", { channel });
      mod.start_c_provider(channel);
      log.info("C provider started (background)", { channel });
    })();
  }
  return bootPromise;
}
