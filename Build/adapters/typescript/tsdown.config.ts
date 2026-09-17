import { defineConfig } from "tsdown";

const outExtensions = ({ format }: { format: string }) =>
  format === "es" ? { js: ".mjs", dts: ".d.mts" } : { js: ".js", dts: ".d.ts" };

export default defineConfig([
  // Library bundle (CJS + ESM + types)
  {
    entry: ["src/index.ts"],
    format: ["cjs", "esm"],
    dts: true,
    sourcemap: true,
    clean: true,
    treeshake: true,
    platform: "node",
    target: "es2022",
    outExtensions,
    deps: { neverBundle: ["net"] },
  },
  // Schema extractor (separate chunk, pulls in the full TypeScript compiler)
  // Import via: import { extractSchema } from "@nisoku/saikuro/schema-extractor"
  {
    entry: { schema_extractor: "src/schema_extractor.ts" },
    format: ["cjs", "esm"],
    dts: true,
    sourcemap: true,
    clean: false,
    treeshake: true,
    platform: "node",
    target: "es2022",
    outExtensions,
  },
  // CLI binary (CommonJS, executable)
  {
    entry: { "cli/saikuro-schema": "src/cli/saikuro-schema.ts" },
    format: ["cjs"],
    dts: false,
    sourcemap: false,
    clean: false,
    treeshake: true,
    platform: "node",
    target: "es2022",
    outExtensions,
    deps: { neverBundle: ["net"] },
    banner: {
      js: "#!/usr/bin/env node",
    },
  },
]);
