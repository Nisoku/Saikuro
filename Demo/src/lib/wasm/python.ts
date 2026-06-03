import { getLogger } from "@nisoku/saikuro";

const log = getLogger("demo.wasm.python");

type Pyodide = {
  runPythonAsync: (code: string) => Promise<any>;
  globals: {
    get: (name: string) => any;
    set: (name: string, value: any) => void;
  };
};

let pyodidePromise: Promise<Pyodide> | null = null;
let scriptLoaded: boolean = false;

export async function loadPythonViz(): Promise<
  (
    stats: Record<string, unknown>,
    ngrams: Record<string, unknown>,
    sentiment: Record<string, unknown>,
  ) => Promise<any>
> {
  if (!pyodidePromise) {
    pyodidePromise = (async () => {
      log.info("loading Pyodide from CDN");
      const module = await import(
        /* @vite-ignore */ "https://cdn.jsdelivr.net/pyodide/v0.26.1/full/pyodide.js"
      );
      const loadPyodide = module.loadPyodide ?? window.loadPyodide;
      if (!loadPyodide) {
        throw new Error("loadPyodide not available");
      }
      log.info("starting Pyodide runtime");
      const pyod = (await loadPyodide({
        indexURL: "https://cdn.jsdelivr.net/pyodide/v0.26.1/full/",
      })) as Pyodide;
      log.info("Pyodide runtime ready");
      return pyod;
    })();
  }

  const pyodide = await pyodidePromise;
  if (!scriptLoaded) {
    log.info("loading Python insight.py script");
    const code = await fetch("/public/wasm/python/insight.py").then((res) =>
      res.text(),
    );
    await pyodide.runPythonAsync(code);
    scriptLoaded = true;
    log.info("Python viz function ready");
  }

  return async (stats, ngrams, sentiment) => {
    // Pyodide passes JS objects as JsProxy which lack .get() and other dict
    // methods.  Round-trip through JSON so the Python code receives proper dicts.
    pyodide.globals.set("__s", JSON.stringify(stats));
    pyodide.globals.set("__n", JSON.stringify(ngrams));
    pyodide.globals.set("__m", JSON.stringify(sentiment));
    const result = await pyodide.runPythonAsync(
      "import json\nprepare_viz(json.loads(__s), json.loads(__n), json.loads(__m))",
    );
    if (result && typeof result.toJs === "function") {
      return result.toJs({ dict_converter: Object });
    }
    return result;
  };
}
