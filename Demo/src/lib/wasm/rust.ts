let rustPromise: Promise<void> | null = null;

export async function startRustProvider(channel: string): Promise<void> {
  if (!rustPromise) {
    rustPromise = (async () => {
      const mod = await import("../../wasm/rust/saikuro_rust_insight.js");
      await mod.default();
      await mod.start_rust_provider(channel);
    })();
  }
  return rustPromise;
}
