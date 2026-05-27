import { defineConfig } from "vite";

export default defineConfig({
    base: "./",
    server: {
        port: 5173,
    },
    build: {
        target: "es2022",
    },
});
