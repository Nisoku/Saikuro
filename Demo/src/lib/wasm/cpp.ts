import { getLogger } from "@nisoku/saikuro";

const log = getLogger("demo.wasm.cpp");

let bootPromise: Promise<void> | null = null;

export async function startCppProvider(channel: string): Promise<void> {
  if (!bootPromise) {
    bootPromise = (async () => {
      log.info("loading C++ WASM", { channel });
      const mod = await import(
        new URL("wasm/cpp/saikuro_cpp_insight.js", document.baseURI).href,
      );
      log.info("C++ WASM module loaded, initializing");
      await mod.default();
      log.info("C++ WASM initialized, starting provider", { channel });
      mod.start_cpp_provider(channel);
      log.info("C++ provider started (background)", { channel });
    })();
  }
  return bootPromise;
}
