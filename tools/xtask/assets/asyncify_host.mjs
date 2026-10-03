// Node host for Asyncify (`xtask test wasm-asyncify`).

import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const outDir = path.resolve(process.argv[2]);
const repetitions = Number(process.argv[3] ?? "10");
const request = "asyncify-probe";

if (!Number.isInteger(repetitions) || repetitions < 1) {
  console.error(`invalid repetition count: ${process.argv[3]}`);
  process.exit(2);
}

const gluePath = path.join(outDir, "saikuro_tests.js");
const glue = await import(pathToFileURL(gluePath).href);
const module = new WebAssembly.Module(
  readFileSync(path.join(outDir, "saikuro_tests_bg.wasm")),
);

glue.initSync({ module });

for (let i = 0; i < repetitions; i++) {
  const report = await glue.callSync(request);
  const echoed = report.match(/asyncify_request=(.*)/);
  if (echoed === null || echoed[1] !== request) {
    console.error(
      `run ${i + 1}: entry did not echo the request; got ${JSON.stringify(echoed?.[1])}\n${report}`,
    );
    process.exit(1);
  }
  const failures = report.match(/asyncify_failed=(\d+)/);
  if (failures === null) {
    console.error(`run ${i + 1}: report has no asyncify_failed marker\n${report}`);
    process.exit(1);
  }
  if (failures[1] !== "0") {
    console.error(`run ${i + 1}: ${failures[1]} test(s) failed\n${report}`);
    process.exit(1);
  }
  const totals = report.match(/Results: (\d+) passed, (\d+) skipped, (\d+) failed/);
  if (totals === null) {
    console.error(`run ${i + 1}: report has no Results line\n${report}`);
    process.exit(1);
  }
  if (i === 0) {
    console.log(
      `  ${totals[1]} passed, ${totals[2]} skipped, ${totals[3]} failed per run`,
    );
  }

  if (i + 1 === repetitions) {
    console.log(`  ${repetitions}/${repetitions} asyncify runs clean`);
  }
}
