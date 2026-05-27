type Pyodide = {
  runPythonAsync: (code: string) => Promise<any>;
  globals: { get: (name: string) => any };
};

let pyodidePromise: Promise<Pyodide> | null = null;
let vizFn: ((stats: any, ngrams: any, sentiment: any) => any) | null = null;

export async function loadPythonViz(): Promise<
  (stats: Record<string, unknown>, ngrams: Record<string, unknown>, sentiment: Record<string, unknown>) => Promise<any>
> {
  if (!pyodidePromise) {
    pyodidePromise = (async () => {
      const module = await import(
        /* @vite-ignore */ "https://cdn.jsdelivr.net/pyodide/v0.26.1/full/pyodide.js"
      );
      const loadPyodide = module.loadPyodide ?? window.loadPyodide;
      if (!loadPyodide) {
        throw new Error("loadPyodide not available");
      }
      return (await loadPyodide({
        indexURL: "https://cdn.jsdelivr.net/pyodide/v0.26.1/full/",
      })) as Pyodide;
    })();
  }

  const pyodide = await pyodidePromise;
  if (!vizFn) {
    const code = await fetch("/wasm/python/insight.py").then((res) => res.text());
    await pyodide.runPythonAsync(code);
    vizFn = pyodide.globals.get("prepare_viz");
  }

  return async (stats, ngrams, sentiment) => {
    const result = vizFn?.(stats, ngrams, sentiment);
    if (result && typeof result.toJs === "function") {
      return result.toJs({ dict_converter: Object });
    }
    return result;
  };
}
