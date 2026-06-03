import { getLogger } from "@nisoku/saikuro";

const log = getLogger("demo.wasm.cpp");

type CppModule = {
  _insight_cpp_ngrams: (inputPtr: number, topN: number) => number;
  _insight_cpp_free: (ptr: number) => void;
  stringToUTF8: (input: string, ptr: number, max: number) => void;
  lengthBytesUTF8: (input: string) => number;
  _malloc: (size: number) => number;
  _free: (ptr: number) => void;
  UTF8ToString: (ptr: number) => string;
};

type NgramResult = {
  bigrams: Array<[string, number]>;
  trigrams: Array<[string, number]>;
};

let cached: Promise<(text: string, topN: number) => NgramResult> | null = null;

export async function loadCppNgrams(): Promise<(text: string, topN: number) => NgramResult> {
  if (!cached) {
    cached = (async () => {
      log.info("loading C++ WASM");
      const moduleFactory = (await import("../../wasm/cpp/insight_cpp.js"))
        .default as () => Promise<CppModule>;
      log.info("C++ WASM module loaded, instantiating");
      const mod = await moduleFactory();
      log.info("C++ WASM instantiated");
      return (text: string, topN: number) => {
        const len = mod.lengthBytesUTF8(text) + 1;
        const ptr = mod._malloc(len);
        try {
          mod.stringToUTF8(text, ptr, len);
          const outPtr = mod._insight_cpp_ngrams(ptr, topN);
          const json = mod.UTF8ToString(outPtr);
          mod._insight_cpp_free(outPtr);
          return JSON.parse(json) as NgramResult;
        } finally {
          mod._free(ptr);
        }
      };
    })();
  }
  return cached;
}
