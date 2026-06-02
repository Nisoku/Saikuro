type DotnetRuntime = {
  getAssemblyExports: (assembly: string) => Promise<any>;
  dispose: () => void;
};

let csharpPromise: Promise<(payload: Record<string, unknown>) => Promise<any>> | null = null;

export async function loadCSharpSummary(): Promise<
  (payload: Record<string, unknown>) => Promise<any>
> {
  if (!csharpPromise) {
    csharpPromise = (async () => {
      const dotnetModule = await import(
        "../../wasm/csharp/dotnet.js"
      );
      const runtime = (await dotnetModule.createDotnetRuntime({
        locateFile: (path: string) => `/wasm/csharp/${path}`,
      })) as DotnetRuntime;

      const exports = await runtime.getAssemblyExports("InsightLab.dll");
      const summarizer = exports.InsightLab.SummaryEngine;

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
