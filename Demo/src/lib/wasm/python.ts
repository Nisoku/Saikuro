import { getLogger } from "@nisoku/saikuro";

const log = getLogger("demo.wasm.python");

let bootPromise: Promise<void> | null = null;

export async function startPythonProvider(channel: string): Promise<void> {
  if (!bootPromise) {
    bootPromise = (async () => {
      log.info("loading Pyodide from CDN", { channel });
      const module = await import(
        /* @vite-ignore */ "https://cdn.jsdelivr.net/pyodide/v0.26.1/full/pyodide.js"
      );
      const loadPyodide = module.loadPyodide ?? (window as any).loadPyodide;
      if (!loadPyodide) throw new Error("loadPyodide not available");

      log.info("starting Pyodide runtime");
      const pyodide = await loadPyodide({
        indexURL: "https://cdn.jsdelivr.net/pyodide/v0.26.1/full/",
      });
      log.info("Pyodide runtime ready");

      log.info("installing packages");
      await pyodide.loadPackage("micropip");
      const micropip = pyodide.pyimport("micropip");

      // Install pure-Python msgpack wheel first (msgpack on PyPI has no py3-none-any.whl)
      const msgpackWheelUrl = new URL(
        "wasm/python/msgpack-1.2.1-py3-none-any.whl",
        document.baseURI,
      ).href;
      await micropip.install(msgpackWheelUrl);

      const wheelUrl = new URL(
        "wasm/python/saikuro-0.1.0-py3-none-any.whl",
        document.baseURI,
      ).href;
      await micropip.install(wheelUrl);
      log.info("packages installed");

      log.info("loading Python insight.py script");
      const insightUrl = new URL(
        "wasm/python/insight.py",
        document.baseURI,
      ).href;
      const resp = await fetch(insightUrl);
      if (!resp.ok) {
        throw new Error(`Failed to fetch insight.py: ${resp.status}`);
      }
      const code = await resp.text();

      // Set the channel argument before running the script
      await pyodide.runPythonAsync(`
import sys
sys.argv = ["insight.py", ${JSON.stringify(channel)}]
      `);

      // Run the insight.py provider (uses SaikuroProvider internally)
      await pyodide.runPythonAsync(code);
      log.info("Python provider started", { channel });
    })();
  }
  return bootPromise;
}
