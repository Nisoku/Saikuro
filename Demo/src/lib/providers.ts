import {
  getLogger,
  SaikuroProvider,
  WasmHostConnector,
  t,
} from "@nisoku/saikuro";
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
  cProvider.register(
    "stats",
    async (...args: unknown[]) => cStats(args[0] as string),
    {
      doc: "Byte-level character statistics (C WASM)",
      args: [{ name: "text", type: t.string() }],
      returns: t.map(t.string(), t.any()),
    },
  );

  const cppProvider = new SaikuroProvider("cpp");
  cppProvider.register(
    "ngrams",
    async (...args: unknown[]) =>
      cppNgrams(args[0] as string, args[1] as number),
    {
      doc: "Tokenization and n-gram counts (C++ WASM)",
      args: [
        { name: "text", type: t.string() },
        { name: "topN", type: t.i32() },
      ],
      returns: t.map(t.string(), t.any()),
    },
  );

  const csharpProvider = new SaikuroProvider("csharp");
  csharpProvider.register(
    "summary",
    async (...args: unknown[]) => {
      return csharpSummary(args[0] as Record<string, unknown>);
    },
    {
      doc: "Business summary logic (C# WASM)",
      args: [{ name: "payload", type: t.any() }],
      returns: t.map(t.string(), t.any()),
    },
  );

  const pythonProvider = new SaikuroProvider("python");
  pythonProvider.register(
    "viz",
    async (...args: unknown[]) => {
      return pythonViz(
        args[0] as Record<string, unknown>,
        args[1] as Record<string, unknown>,
        args[2] as Record<string, unknown>,
      );
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

  for (const p of [cProvider, cppProvider, csharpProvider, pythonProvider]) {
    const promise = serveProvider(p, channel).catch((err) =>
      log.warn("provider failed to serve", { provider: p.namespace, err })
,
    );
    activeProviders.push(promise);
  }

  log.info("all providers serving");
}

async function serveProvider(
  provider: SaikuroProvider,
  channel: string,
): Promise<void> {
  log.info("connecting provider", { namespace: provider.namespace, channel });
  const connector = new WasmHostConnector(channel);
  const transport = await connector.connect();
  log.info("provider transport connected, serving", {
    namespace: provider.namespace,
  });
  // serveOn enters the dispatch loop and never resolves.
  provider.serveOn(transport).catch((err) =>
  log.warn("provider dispatch failed", { provider: provider.namespace, err })
);
}
