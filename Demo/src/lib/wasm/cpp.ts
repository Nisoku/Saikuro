type CppModule = {
  _insight_cpp_ngrams: (inputPtr: number, topN: number) => number;
  _insight_cpp_free: (ptr: number) => void;
  stringToUTF8: (input: string, ptr: number, max: number) => void;
  lengthBytesUTF8: (input: string) => number;
  _malloc: (size: number) => number;
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
      const moduleFactory = (await import("../../wasm/cpp/insight_cpp.js"))
        .default as () => Promise<CppModule>;
      const mod = await moduleFactory();
      return (text: string, topN: number) => {
        const len = mod.lengthBytesUTF8(text) + 1;
        const ptr = mod._malloc(len);
        mod.stringToUTF8(text, ptr, len);
        const outPtr = mod._insight_cpp_ngrams(ptr, topN);
        const json = mod.UTF8ToString(outPtr);
        mod._insight_cpp_free(outPtr);
        return JSON.parse(json) as NgramResult;
      };
    })();
  }
  return cached;
}
