import { SaikuroProvider, WasmHostConnector, t } from "@nisoku/saikuro";
import { loadCStats } from "./wasm/c";
import { loadCppNgrams } from "./wasm/cpp";
import { loadCSharpSummary } from "./wasm/csharp";
import { loadPythonViz } from "./wasm/python";

let started = false;
const activeProviders: Promise<void>[] = [];

export async function startProviders(channel: string): Promise<void> {
  if (started) return;
  started = true;

  const [cStats, cppNgrams, csharpSummary, pythonViz] = await Promise.all([
    loadCStats(),
    loadCppNgrams(),
    loadCSharpSummary(),
    loadPythonViz(),
  ]);

  const cProvider = new SaikuroProvider("c");
  cProvider.register("stats", async (text: string) => cStats(text), {
    doc: "Byte-level character statistics (C WASM)",
    args: [{ name: "text", type: t.string() }],
    returns: t.map(t.string(), t.any()),
  });

  const cppProvider = new SaikuroProvider("cpp");
  cppProvider.register("ngrams", async (text: string, topN: number) => cppNgrams(text, topN), {
    doc: "Tokenization and n-gram counts (C++ WASM)",
    args: [
      { name: "text", type: t.string() },
      { name: "topN", type: t.i32() },
    ],
    returns: t.map(t.string(), t.any()),
  });

  const csharpProvider = new SaikuroProvider("csharp");
  csharpProvider.register("summary", async (payload: Record<string, unknown>) => {
    return csharpSummary(payload);
  }, {
    doc: "Business summary logic (C# WASM)",
    args: [{ name: "payload", type: t.any() }],
    returns: t.map(t.string(), t.any()),
  });

  const pythonProvider = new SaikuroProvider("python");
  pythonProvider.register(
    "viz",
    async (stats: Record<string, unknown>, ngrams: Record<string, unknown>, sentiment: Record<string, unknown>) => {
      return pythonViz(stats, ngrams, sentiment);
    },
    {
      doc: "Visualization prep (Pyodide)",
      args: [
        { name: "stats", type: t.any() },
        { name: "ngrams", type: t.any() },
        { name: "sentiment", type: t.any() },
      ],
      returns: t.map(t.string(), t.any()),
    },
  );

  activeProviders.push(
    serveProvider(cProvider, channel),
    serveProvider(cppProvider, channel),
    serveProvider(csharpProvider, channel),
    serveProvider(pythonProvider, channel),
  );
}

async function serveProvider(provider: SaikuroProvider, channel: string): Promise<void> {
  const connector = new WasmHostConnector(channel);
  const transport = await connector.connect();
  await provider.serveOn(transport);
}
