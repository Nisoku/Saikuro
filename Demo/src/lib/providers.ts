import { getLogger, SaikuroProvider, WasmHostConnector, t } from "@nisoku/saikuro";
import { loadCStats } from "./wasm/c";
import { loadCppNgrams } from "./wasm/cpp";
import { loadCSharpSummary } from "./wasm/csharp";
import { loadPythonViz } from "./wasm/python";

const log = getLogger("demo.providers");

let started = false;
const activeProviders: Promise<void>[] = [];

export async function startProviders(channel: string): Promise<void> {
  if (started) return;
  started = true;

  log.info("loading all WASM modules in parallel");

  const [cStats, cppNgrams, csharpSummary, pythonViz] = await Promise.all([
    loadCStats(),
    loadCppNgrams(),
    loadCSharpSummary(),
    loadPythonViz(),
  ]);

  log.info("all WASM modules loaded, registering providers");

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

  log.info("all providers serving");
}

async function serveProvider(provider: SaikuroProvider, channel: string): Promise<void> {
  log.info("connecting provider", { namespace: provider.namespace, channel });
  const connector = new WasmHostConnector(channel);
  const transport = await connector.connect();
  log.info("provider transport connected, serving", { namespace: provider.namespace });
  // serveOn enters the dispatch loop and never resolves.
  provider.serveOn(transport);
}
