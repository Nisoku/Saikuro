let runtimePromise: Promise<void> | null = null;

export async function startRuntimeWasm(channel: string): Promise<void> {
  if (!runtimePromise) {
    runtimePromise = (async () => {
      const mod = await import("../../wasm/runtime/saikuro_web_runtime.js");
      await mod.default();
      await mod.start_runtime(channel);
    })();
  }
  return runtimePromise;
}
