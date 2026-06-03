import { getLogger } from "@nisoku/saikuro";

const log = getLogger("demo.wasm.csharp");

let csharpPromise: Promise<(payload: Record<string, unknown>) => Promise<any>> | null = null;

export async function loadCSharpSummary(): Promise<
  (payload: Record<string, unknown>) => Promise<any>
> {
  if (!csharpPromise) {
    csharpPromise = (async () => {
      log.info("loading C# WASM (dotnet.js)");
      const { default: createDotnetRuntime } = await import(
        "../../wasm/csharp/dotnet.js"
      );
      log.info("C# dotnet.js loaded, creating runtime");
      const runtime = await createDotnetRuntime({
        locateFile: (path: string) => `/wasm/csharp/${path}`,
      } as any);
      log.info("C# runtime created, getting assembly exports");

      const exports = await runtime.getAssemblyExports("InsightLab.dll");
      const summarizer = exports.InsightLab.SummaryEngine;
      log.info("C# assembly exports ready");

      return async (payload: Record<string, unknown>) => {
        const json = JSON.stringify(payload);
        const result = summarizer.Summarize(json);
        if (typeof result === "string") {
          return JSON.parse(result);
        }
        return result;
      };
    })();
  }
  return csharpPromise;
}
