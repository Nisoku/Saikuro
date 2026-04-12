import { describe, it, expect } from "vitest";
import { extractSchema } from "../src/schema_extractor";
import {
  canonType,
  canonFn,
  normalizeReturns,
  tolerantNormalize,
} from "./canonicalize";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { spawnSync } from "child_process";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

describe("Schema parity: TypeScript ↔ Python (basic)", () => {
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
    const res = spawnSync("python3", [pyFixture], {
      encoding: "utf-8",
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
      "dotnet",
      [
        "run",
        "--project",
        resolve(__dirname, "../../csharp/tools/extractor/extractor.csproj"),
        "parityns",
      ],
      { encoding: "utf-8" },
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

      const norm = normalizeAnyComplex(a.returns, b.returns);
      if (norm) {
        b.returns = norm;
      }
      const norm2 = normalizeAnyComplex(a.returns, c.returns);
      if (norm2) {
        c.returns = norm2;
      }
      const norm3 = normalizeAnyComplex(b.returns, a.returns);
      if (norm3) {
        a.returns = norm3;
      }
      const norm4 = normalizeAnyComplex(b.returns, c.returns);
      if (norm4) {
        c.returns = norm4;
      }
      const norm5 = normalizeAnyComplex(c.returns, a.returns);
      if (norm5) {
        a.returns = norm5;
      }
      const norm6 = normalizeAnyComplex(c.returns, b.returns);
      if (norm6) {
        b.returns = norm6;
      }

      // Also normalize argument types where one side reports `any` but
      // another reports structured list/map/stream.
      const argCount = Math.max(a.args.length, b.args.length, c.args.length);
      for (let i = 0; i < argCount; i++) {
        const ta = a.args[i] ? a.args[i][1] : null;
        const tb = b.args[i] ? b.args[i][1] : null;
        const tc = c.args[i] ? c.args[i][1] : null;
        const anySide = isAny(ta) || isAny(tb) || isAny(tc);
        const complexSide =
          isList(ta) ||
          isMap(ta) ||
          isStream(ta) ||
          isList(tb) ||
          isMap(tb) ||
          isStream(tb) ||
          isList(tc) ||
          isMap(tc) ||
          isStream(tc);
        if (anySide && complexSide) {
          if (a.args[i]) a.args[i][1] = ["p", "any"];
          if (b.args[i]) b.args[i][1] = ["p", "any"];
          if (c.args[i]) c.args[i][1] = ["p", "any"];
        }
      }

      if (
        JSON.stringify(a) !== JSON.stringify(b) ||
        JSON.stringify(c) !== JSON.stringify(b)
      ) {
        console.error("Mismatch for function:", name);
        console.error("TS:", JSON.stringify(a, null, 2));
        console.error("PY:", JSON.stringify(b, null, 2));
        console.error("CS:", JSON.stringify(c, null, 2));
      }
      expect(a).toEqual(b);
      expect(c).toEqual(b);
    }
    },
    60000,
  );
});
