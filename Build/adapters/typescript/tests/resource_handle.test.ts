/**
 * Tests for decodeResourceHandle
 *
 */

import { describe, it, expect } from "vitest";
import { decodeResourceHandle } from "../src/envelope";
import type { ResourceHandle } from "../src/envelope";

describe("decodeResourceHandle", () => {
  it("decodes a minimal handle with only id", () => {
    expect(decodeResourceHandle({ id: "res-1" })).toEqual({ id: "res-1" });
  });

  it("decodes a fully-populated handle", () => {
    const raw = {
      id: "res-2",
      mime_type: "image/png",
      size: 4096,
      uri: "saikuro://res/res-2",
    };
    expect(decodeResourceHandle(raw)).toEqual<ResourceHandle>({
      id: "res-2",
      mime_type: "image/png",
      size: 4096,
      uri: "saikuro://res/res-2",
    });
  });

  it("returns null for null input", () => {
    expect(decodeResourceHandle(null)).toBeNull();
  });

  it("returns null for undefined input", () => {
    expect(decodeResourceHandle(undefined)).toBeNull();
  });

  it("returns null for a string input", () => {
    expect(decodeResourceHandle("not-an-object")).toBeNull();
  });

  it("returns null when id field is missing", () => {
    expect(decodeResourceHandle({ mime_type: "text/plain" })).toBeNull();
  });

  it("returns null when id is not a string", () => {
    expect(decodeResourceHandle({ id: 42 })).toBeNull();
  });

  it("omits optional fields that are absent from the raw object", () => {
    const h = decodeResourceHandle({ id: "x" }) as ResourceHandle;
    expect("mime_type" in h).toBe(false);
    expect("size" in h).toBe(false);
    expect("uri" in h).toBe(false);
  });

  it("drops unknown extra fields", () => {
    const h = decodeResourceHandle({
      id: "x",
      unknown_field: true,
    }) as unknown as Record<string, unknown>;
    expect(h["unknown_field"]).toBeUndefined();
  });
});
