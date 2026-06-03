import { defineConfig } from "tsup";

export default defineConfig([
  // Library bundle (CJS + ESM + types)
  {
    entry: ["src/index.ts"],
    format: ["cjs", "esm"],
    dts: true,
    sourcemap: true,
    clean: true,
    splitting: true,
    treeshake: true,
    platform: "node",
    target: "es2022",
    external: ["net"],
    esbuildOptions(options) {
      options.pure = ["console.log"];
    },
  },
  // Schema extractor (separate chunk, pulls in the full TypeScript compiler)
  // Import via: import { extractSchema } from "@nisoku/saikuro/schema-extractor"
  {
    entry: { schema_extractor: "src/schema_extractor.ts" },
    format: ["cjs", "esm"],
    dts: true,
    sourcemap: true,
    clean: false,
    splitting: false,
    treeshake: true,
    platform: "node",
    target: "es2022",
  },
  // CLI binary (CommonJS, executable)
  {
    entry: { "cli/saikuro-schema": "src/cli/saikuro-schema.ts" },
    format: ["cjs"],
    dts: false,
    sourcemap: false,
    splitting: false,
    treeshake: true,
    platform: "node",
    target: "es2022",
    external: ["net"],
    banner: {
      js: "#!/usr/bin/env node",
    },
  },
]);
