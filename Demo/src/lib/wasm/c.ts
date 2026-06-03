import { getLogger } from "@nisoku/saikuro";

const log = getLogger("demo.wasm.c");

type CModule = {
  _insight_c_stats: (inputPtr: number) => number;
  _insight_c_free: (ptr: number) => void;
  stringToUTF8: (input: string, ptr: number, max: number) => void;
  lengthBytesUTF8: (input: string) => number;
  _malloc: (size: number) => number;
  UTF8ToString: (ptr: number) => string;
};

type StatsResult = {
  bytes: number;
  chars: number;
  ascii: number;
  non_ascii: number;
};

let cached: Promise<(text: string) => StatsResult> | null = null;

export async function loadCStats(): Promise<(text: string) => StatsResult> {
  if (!cached) {
    cached = (async () => {
      log.info("loading C WASM");
      const moduleFactory = (await import("../../wasm/c/insight_c.js"))
        .default as () => Promise<CModule>;
      log.info("C WASM module loaded, instantiating");
      const mod = await moduleFactory();
      log.info("C WASM instantiated");
      return (text: string) => {
        const len = mod.lengthBytesUTF8(text) + 1;
        const ptr = mod._malloc(len);
        mod.stringToUTF8(text, ptr, len);
        const outPtr = mod._insight_c_stats(ptr);
        const json = mod.UTF8ToString(outPtr);
        mod._insight_c_free(outPtr);
        return JSON.parse(json) as StatsResult;
      };
    })();
  }
  return cached;
}
