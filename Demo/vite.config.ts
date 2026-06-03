import { defineConfig } from "vite";

export default defineConfig({
  base: "./",
  server: {
    port: 5173,
  },
  resolve: {
    alias: {
      env: "/src/env.ts",
    },
  },
  build: {
    target: "es2022",
  },
});
