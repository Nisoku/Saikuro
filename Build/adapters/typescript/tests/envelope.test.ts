/**
 * Tests for envelope factory functions
 *
 */

import { describe, it, expect } from "vitest";
import {
  makeCallEnvelope,
  makeCastEnvelope,
  makeStreamOpenEnvelope,
  makeChannelOpenEnvelope,
  makeResourceEnvelope,
  makeAnnounceEnvelope,
  PROTOCOL_VERSION,
} from "../src/envelope";
import type { SaikuroSchema } from "../src/envelope";

describe("makeCallEnvelope", () => {
  it("sets version to PROTOCOL_VERSION", () => {
    expect(makeCallEnvelope("math.add", [1, 2]).version).toBe(PROTOCOL_VERSION);
  });

  it("sets type to 'call'", () => {
    expect(makeCallEnvelope("math.add", [1, 2]).type).toBe("call");
  });

  it("sets target and args", () => {
    const env = makeCallEnvelope("math.add", [1, 2]);
    expect(env.target).toBe("math.add");
    expect(env.args).toEqual([1, 2]);
  });

  it("generates a unique id on each call", () => {
    const a = makeCallEnvelope("fn", []);
    const b = makeCallEnvelope("fn", []);
    expect(a.id).not.toBe(b.id);
  });

  it("includes capability when provided", () => {
    const env = makeCallEnvelope("math.add", [], "tok-123");
    expect(env.capability).toBe("tok-123");
  });

  it("does not include capability key when omitted", () => {
    const env = makeCallEnvelope("math.add", []);
    expect("capability" in env).toBe(false);
  });
});

describe("makeCastEnvelope", () => {
  it("sets type to 'cast'", () => {
    expect(makeCastEnvelope("log.info", ["hello"]).type).toBe("cast");
  });

  it("preserves target and args", () => {
    const env = makeCastEnvelope("log.info", ["hello"]);
    expect(env.target).toBe("log.info");
    expect(env.args).toEqual(["hello"]);
  });
});

describe("makeStreamOpenEnvelope", () => {
  it("sets type to 'stream'", () => {
    expect(makeStreamOpenEnvelope("events.subscribe", []).type).toBe("stream");
  });
});

describe("makeChannelOpenEnvelope", () => {
  it("sets type to 'channel'", () => {
    expect(makeChannelOpenEnvelope("chat.open", []).type).toBe("channel");
  });
});

describe("makeResourceEnvelope", () => {
  it("sets type to 'resource'", () => {
    expect(makeResourceEnvelope("files.open", ["/tmp/x"]).type).toBe(
      "resource",
    );
  });

  it("forwards capability", () => {
    const env = makeResourceEnvelope("files.open", [], "cap-abc");
    expect(env.capability).toBe("cap-abc");
  });
});

describe("makeAnnounceEnvelope", () => {
  const schema: SaikuroSchema = { version: 1, namespaces: {}, types: {} };

  it("sets type to 'announce'", () => {
    expect(makeAnnounceEnvelope(schema).type).toBe("announce");
  });

  it("sets target to $saikuro.announce", () => {
    expect(makeAnnounceEnvelope(schema).target).toBe("$saikuro.announce");
  });

  it("embeds the schema in args[0]", () => {
    expect(makeAnnounceEnvelope(schema).args[0]).toEqual(schema);
  });
});
