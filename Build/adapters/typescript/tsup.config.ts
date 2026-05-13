import { defineConfig } from "tsup";

export default defineConfig([
  // Library bundle (CJS + ESM + types)
  {
    entry: ["src/index.ts"],
    format: ["cjs", "esm"],
    dts: true,
    sourcemap: true,
    clean: true,
    splitting: false,
    treeshake: true,
    platform: "node",
    target: "es2022",
    external: ["net"],
    esbuildOptions(options) {
      options.pure = ["console.log"];
    },
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
