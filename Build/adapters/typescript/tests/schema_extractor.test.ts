import { describe, it, expect } from "vitest";
import { extractSchema } from "../src/schema_extractor";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

describe("SchemaExtractor", () => {
  it("extracts functions from fixture service", async () => {
    const file = resolve(__dirname, "./fixtures/service.ts");
    const schema = await extractSchema([file], "testns");

    // Basic assertions about structure
    expect(schema).toHaveProperty("version");
    expect(schema).toHaveProperty("namespaces");
    const ns = (schema as any).namespaces["testns"];
    expect(ns).toBeTruthy();
    expect(ns.functions).toHaveProperty("add");
    expect(ns.functions).toHaveProperty("gen_numbers");
    expect(ns.functions).toHaveProperty("maybe");

    const add = ns.functions.add;
    expect(add.args.length).toBe(2);
    expect(add.returns).toBeTruthy();
    expect(add.capabilities).toContain("calc");
  });
});
