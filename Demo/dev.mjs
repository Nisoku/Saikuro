#!/usr/bin/env node

/**
 * Usage:
 *   node dev.mjs           # initial build + vite + file watching
 *   node dev.mjs build     # one-shot build (no watcher)
 *   node dev.mjs --help    # this message
 */

import fs from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";

const ROOT = path.dirname(new URL(import.meta.url).pathname);
const WASM = path.join(ROOT, "wasm");

// why am i redoing this and not using tuiro and python? wellll idk. oops.
const BOLD = "\x1b[1m";
const CYAN = "\x1b[36m";
const GREEN = "\x1b[32m";
const YELLOW = "\x1b[33m";
const RED = "\x1b[31m";
const RESET = "\x1b[0m";
const CLEAR = "\x1bc";

function banner(msg) {
  console.log(`\n${CYAN}- ${msg}${RESET}`);
}
function ok(msg) {
  console.log(`${GREEN}✔ ${msg}${RESET}`);
}
function warn(msg) {
  console.log(`${YELLOW}⚠ ${msg}${RESET}`);
}

// just runner
function just(recipe) {
  return new Promise((resolve, reject) => {
    const child = spawn("just", [recipe], {
      stdio: "inherit",
      cwd: ROOT,
    });
    child.on("exit", (code) => {
      if (code === 0) resolve();
      else reject(new Error(`just ${recipe} exited ${code}`));
    });
    child.on("error", (e) => reject(new Error(`just ${recipe} failed: ${e.message}`)));
  });
}

// initial build
async function initialBuild() {
  process.stdout.write(CLEAR);
  console.log(`${BOLD}╔═══════════════════════════════════════════════════════════╗${RESET}`);
  console.log(`${BOLD}║     Saikuro Insight Lab initial build                     ║${RESET}`);
  console.log(`${BOLD}╚═══════════════════════════════════════════════════════════╝${RESET}`);

  const steps = [
    "wasm-rust-runtime",
    "wasm-rust-provider",
    "wasm-c",
    "wasm-cpp",
    "wasm-csharp",
    "wasm-python",
  ];

  for (const recipe of steps) {
    const label = recipe.replace("wasm-", "").replace("-", " ");
    const start = Date.now();
    process.stdout.write(`  ${label} … `);
    try {
      await just(recipe);
    } catch (e) {
      warn(`${label} failed: ${e.message}`);
    }
    const elapsed = ((Date.now() - start) / 1000).toFixed(1);
    console.log(`  ${label} done in ${elapsed}s\n`);
  }

  ok("Initial build complete");
}

// file watcher
const debounceTimers = {};

function scheduleBuild(target, recipe, delay = 400) {
  if (debounceTimers[target]) clearTimeout(debounceTimers[target]);
  debounceTimers[target] = setTimeout(() => {
    delete debounceTimers[target];
    banner(`Rebuilding ${target}`);
    just(recipe).then(
      () => ok(`${target} rebuilt`),
      (e) => warn(`${target} build failed: ${e.message}`),
    );
  }, delay);
}

function isIgnored(filename) {
  if (!filename) return true;
  const base = path.basename(filename);
  if (base.startsWith(".") || base.endsWith("~") || base.endsWith(".swp")) return true;
  const parts = filename.split(/[/\\]/);
  if (parts.includes("target") || parts.includes("bin") || parts.includes("obj")) return true;
  return false;
}

function watchDir(dir, target, recipe) {
  if (!fs.existsSync(dir)) {
    warn(`Skipping ${path.relative(ROOT, dir)}, not found`);
    return;
  }
  try {
    fs.watch(dir, { recursive: true }, (_event, filename) => {
      if (isIgnored(filename)) return;
      console.log(`  ${YELLOW}◉${RESET} ${target}: ${filename}`);
      scheduleBuild(target, recipe);
    });
    console.log(`  watching ${CYAN}${path.relative(ROOT, dir)}${RESET}/`);
  } catch (e) {
    warn(`Failed to watch ${dir}: ${e.message}`);
  }
}

function watchFile(file, target, recipe) {
  if (!fs.existsSync(file)) {
    warn(`Skipping ${path.relative(ROOT, file)}, not found`);
    return;
  }
  try {
    fs.watch(file, () => {
      const rel = path.relative(ROOT, file);
      console.log(`  ${YELLOW}◉${RESET} ${target}: ${rel}`);
      scheduleBuild(target, recipe);
    });
    console.log(`  watching ${CYAN}${path.relative(ROOT, file)}${RESET}`);
  } catch (e) {
    warn(`Failed to watch ${file}: ${e.message}`);
  }
}

function startWatcher() {
  console.log(`\n${BOLD} Watching for changes${RESET}\n`);

  watchDir(path.join(WASM, "c"),                    "C",       "wasm-c");
  watchDir(path.join(WASM, "cpp"),                  "C++",     "wasm-cpp");
  watchDir(path.join(WASM, "python"),               "Python",  "wasm-python");
  watchDir(path.join(WASM, "rust", "src"),          "Rust",    "wasm-rust-provider");
  watchDir(path.join(WASM, "runtime", "src"),       "Runtime", "wasm-rust-runtime");
  watchDir(path.join(WASM, "csharp", "InsightLab"), "C#",      "wasm-csharp");

  watchFile(path.join(WASM, "rust", "Cargo.toml"),    "Rust",     "wasm-rust-provider");
  watchFile(path.join(WASM, "runtime", "Cargo.toml"), "Runtime",  "wasm-rust-runtime");

  console.log(`\n${BOLD} Ready. Edit a source file to trigger a partial rebuild${RESET}\n`);
}

// main
function usage() {
  console.log(`
Usage: node dev.mjs [command]

Commands:
  dev     (default)  Initial build + start Vite + watch source files
  build              One-shot build all targets (no watcher)
  --help             Show this message
`);
}

async function main() {
  const cmd = process.argv[2] || "dev";

  if (cmd === "--help" || cmd === "-h") {
    usage();
    return;
  }

  if (cmd === "build") {
    await initialBuild();
    return;
  }

  if (cmd !== "dev") {
    console.error(`Unknown command: ${cmd}`);
    usage();
    process.exit(1);
  }

  // dev mode
  await initialBuild();

  // Start Vite dev server
  banner("Starting Vite");
  const vite = spawn("npm", ["run", "dev"], {
    stdio: "inherit",
    cwd: ROOT,
  });

  vite.on("exit", (code) => {
    console.log(`\nVite exited (${code})`);
    process.exit(code ?? 0);
  });

  vite.on("error", (e) => {
    console.error(`Failed to start Vite: ${e.message}`);
    process.exit(1);
  });

  // Start file watcher
  startWatcher();

  // Graceful shutdown
  const onSignal = () => {
    vite.kill("SIGTERM");
    setTimeout(() => process.exit(0), 2000);
  };
  process.on("SIGINT", onSignal);
  process.on("SIGTERM", onSignal);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
