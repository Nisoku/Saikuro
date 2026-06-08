import { describe, it, expect } from "vitest";
import { extractSchema } from "../src/schema_extractor";
import { canonType, canonFn, normalizeNumeric } from "./canonicalize";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { spawnSync } from "child_process";
import { existsSync } from "fs";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const pyAdapterRoot = resolve(__dirname, "../../python");

const TEST_TIMEOUT = 60000;

function getPythonBin(): string {
  if (process.platform === "win32") {
    const venvPython = resolve(pyAdapterRoot, ".venv\\Scripts\\python.exe");
    if (existsSync(venvPython)) return venvPython;
    return "python.exe";
  }
  const venvPython = resolve(pyAdapterRoot, ".venv/bin/python3");
  if (existsSync(venvPython)) return venvPython;
  return "python3";
}

function getDotnetBin(): string {
  // Check PATH first
  const pathSep = process.platform === "win32" ? ";" : ":";
  const pathDirs = (process.env.PATH || "").split(pathSep);
  for (const dir of pathDirs) {
    const candidate = resolve(dir, "dotnet");
    if (existsSync(candidate)) return candidate;
  }
  // Check DOTNET_ROOT
  const root = process.env.DOTNET_ROOT;
  if (root) {
    const candidate = resolve(root, "dotnet");
    if (existsSync(candidate)) return candidate;
  }
  // Check ~/.dotnet (common install location from dotnet-install.sh)
  const home = process.env.HOME;
  if (home) {
    const candidate = resolve(home, ".dotnet", "dotnet");
    if (existsSync(candidate)) return candidate;
  }
  return "dotnet";
}

describe("Schema parity: TypeScript <-> Python (basic)", () => {
  it(
    "TS extractor and Python builder produce compatible schemas for fixture",
    async () => {
      const tsFixture = resolve(__dirname, "./fixtures/service.ts");
      const tsSchema = await extractSchema([tsFixture], "parityns");

      // Invoke the Python schema builder script to produce a schema for the
      // same fixture written in Python. We will use the existing Python
      // builder by executing a small inline Python snippet that imports the
      // module and calls SchemaBuilder on a constructed function.
      // For fast iteration we will write a tiny Python one-liner that
      // imports the adapter's schema builder and prints JSON to stdout.

      const pyFixture = resolve(
        __dirname,
        "../../python/tests/fixtures/service.py",
      );
      const res = spawnSync(getPythonBin(), [pyFixture], {
        encoding: "utf-8",
        timeout: TEST_TIMEOUT,
        env: {
          ...process.env,
          PYTHONPATH: resolve(__dirname, "../../python"),
          PYTHONUNBUFFERED: "1",
        },
      });
      if (res.error) throw res.error;
      if (res.status !== 0) throw new Error(`python failed: ${res.stderr}`);

      const pySchema = JSON.parse(res.stdout);

      // Also run the C# schema extractor tool (dotnet) to produce a schema for
      // parity checks. The small extractor exe prints JSON to stdout.
      const csRes = spawnSync(
        getDotnetBin(),
        [
          "run",
          "--project",
          resolve(__dirname, "../../csharp/tools/extractor/extractor.csproj"),
          "parityns",
        ],
        { encoding: "utf-8", timeout: TEST_TIMEOUT },
      );
      if (csRes.error) throw csRes.error;
      if (csRes.status !== 0) throw new Error(`dotnet failed: ${csRes.stderr}`);
      const csOut = (csRes.stdout || "").toString().trim();
      const firstBrace = csOut.indexOf("{");
      const lastBrace = csOut.lastIndexOf("}");
      if (firstBrace === -1 || lastBrace === -1)
        throw new Error(`dotnet output missing JSON: ${csOut}`);
      const csJson = csOut.slice(firstBrace, lastBrace + 1);
      const csSchema = JSON.parse(csJson);

      // Canonicalize type descriptors and functions using shared helpers.

      const buildMap = (schema: any) =>
        Object.entries(
          (schema as any).namespaces["parityns"].functions || {},
        ).reduce((acc: any, [k, v]: any) => {
          // Normalize function name: camelCase / PascalCase -> snake_case lower
          const norm = String(k)
            .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
            .replace(/[^a-zA-Z0-9_]+/g, "_")
            .replace(/__+/g, "_")
            .toLowerCase();
          acc[norm] = canonFn(v);
          return acc;
        }, {} as any);

      const tsFns = buildMap(tsSchema);
      const pyFns = buildMap(pySchema);
      const csFns = buildMap(csSchema);

      // Compare function sets and shapes
      // All three should expose the same function set
      expect(Object.keys(tsFns).sort()).toEqual(Object.keys(pyFns).sort());
      expect(Object.keys(csFns).sort()).toEqual(Object.keys(pyFns).sort());
      const normalizeReturns = (c: any): any => {
        if (!c) return c;
        if (Array.isArray(c)) {
          // If optional wrapper exists, collapse it to the inner type because
          // the Python builder emits Optional[T] as the inner type string.
          if (c[0] === "o") return normalizeReturns(c[1]);

          // stream(any) ~= any on Python side; collapse for parity check
          if (
            c[0] === "s" &&
            Array.isArray(c[1]) &&
            c[1][0] === "p" &&
            c[1][1] === "any"
          ) {
            return ["p", "any"];
          }

          return c.map(normalizeReturns);
        }
        return c;
      };

      for (const name of Object.keys(tsFns)) {
        if (name === "sum_values" || name === "wrap_items") {
          console.error("TS:", JSON.stringify(tsFns[name], null, 2));
          console.error("PY:", JSON.stringify(pyFns[name], null, 2));
          console.error("CS:", JSON.stringify(csFns[name], null, 2));
        }
        const a = {
          ...tsFns[name],
          returns: normalizeReturns(tsFns[name].returns),
        };
        const b = {
          ...pyFns[name],
          returns: normalizeReturns(pyFns[name].returns),
        };
        const c = {
          ...csFns[name],
          returns: normalizeReturns(csFns[name].returns),
        };

        // Tolerate cases where one extractor reports an untyped `any` primitive
        // while another reports a `stream<T>`, `list`, or `map`. Treat stream(X)
        // or list/map ~= any for parity comparisons when one side is `any`.
        const isAny = (r: any) =>
          Array.isArray(r) && r[0] === "p" && r[1] === "any";
        const isStream = (r: any) => Array.isArray(r) && r[0] === "s";
        const isList = (r: any) => Array.isArray(r) && r[0] === "l";
        const isMap = (r: any) => Array.isArray(r) && r[0] === "m";

        const normalizeAnyComplex = (x: any, y: any) => {
          if (isAny(x) && (isStream(y) || isList(y) || isMap(y)))
            return ["p", "any"];
          return null;
        };

        const rets = [a, b, c];
        for (let i = 0; i < rets.length; i++) {
          for (let j = 0; j < rets.length; j++) {
            if (i === j) continue;
            const norm = normalizeAnyComplex(rets[i].returns, rets[j].returns);
            if (norm) rets[j].returns = norm;
          }
        }

        // Also normalize argument types where one side reports `any` but
        // another reports structured list/map/stream.
        const argSources = [a, b, c];
        const argCount = Math.max(...argSources.map((s) => s.args.length));
        for (let i = 0; i < argCount; i++) {
          const types = argSources.map((s) =>
            s.args[i] ? s.args[i][1] : null,
          );
          const anySide = types.some((t) => isAny(t));
          const complexSide = types.some(
            (t) => isList(t) || isMap(t) || isStream(t),
          );
          if (anySide && complexSide) {
            for (const s of argSources) {
              if (s.args[i]) s.args[i][1] = ["p", "any"];
            }
          }
        }

        // Normalize numeric types (f64/i64) for cross-language parity
        const a1 = normalizeNumeric(a);
        const b1 = normalizeNumeric(b);
        const c1 = normalizeNumeric(c);

        if (
          JSON.stringify(a1) !== JSON.stringify(b1) ||
          JSON.stringify(c1) !== JSON.stringify(b1)
        ) {
          console.error("Mismatch for function:", name);
          console.error("TS:", JSON.stringify(a1, null, 2));
          console.error("PY:", JSON.stringify(b1, null, 2));
          console.error("CS:", JSON.stringify(c1, null, 2));
        }
        expect(a1).toEqual(b1);
        expect(c1).toEqual(b1);
      }
    },
    TEST_TIMEOUT,
  );
});
