import { getLogger } from "@nisoku/saikuro";

const log = getLogger("demo.wasm.rust");

let rustStarted = false;

export async function startRustProvider(channel: string): Promise<void> {
  if (rustStarted) return;
  rustStarted = true;

  log.info("loading Rust WASM", { channel });
  const mod = await import("../../wasm/rust/saikuro_rust_insight.js");
  log.info("Rust WASM module loaded, initializing");
  await mod.default();
  log.info("Rust WASM initialized, starting provider", { channel });

  // start_rust_provider is async and runs serve_on (dispatch loop) forever.
  // Fire it in the background so bootRuntime can proceed.
  void mod.start_rust_provider(channel);
  log.info("Rust provider started (background)", { channel });
}
