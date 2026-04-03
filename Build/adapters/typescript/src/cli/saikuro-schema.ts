#!/usr/bin/env node
import { extractSchema } from "../schema_extractor";
import { resolve } from "path";

async function main() {
  const argv = process.argv.slice(2);
  if (argv.length < 2) {
    console.error(
      "Usage: saikuro-schema <namespace> <file1> [file2...]. Example: saikuro-schema myns src/foo.ts src/bar.ts"
    );
    process.exit(2);
  }

  const namespace = argv[0];
  const files = argv.slice(1).map((f) => resolve(f));

  try {
    const schema = await extractSchema(files, namespace);
    console.log(JSON.stringify(schema, null, 2));
  } catch (err) {
    console.error("Failed to extract schema:", err instanceof Error ? err.message : err);
    process.exit(1);
  }
}

void main();
