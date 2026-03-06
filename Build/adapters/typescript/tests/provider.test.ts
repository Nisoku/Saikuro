/**
 * Tests for SaikuroProvider
 */

import { describe, it, expect } from "vitest";
import { SaikuroProvider, t } from "../src/provider";
import { InMemoryTransport } from "../src/transport";
import { SaikuroError } from "../src/error";
import type { Envelope } from "../src/envelope";
import { PROTOCOL_VERSION } from "../src/envelope";

//  Helpers

/** Build a minimal outbound Envelope. */
function makeEnv(
  type: Envelope["type"],
  target: string,
  args: unknown[],
  id = "test-id",
): Envelope {
  return { version: PROTOCOL_VERSION, type, id, target, args };
}

/** Collect all messages sent to a transport. */
function collectMessages(transport: {
  onMessage: (h: (m: Record<string, unknown>) => void) => void;
}): Record<string, unknown>[] {
  const msgs: Record<string, unknown>[] = [];
  transport.onMessage((m) => msgs.push(m));
  return msgs;
}

//  namespace / register

describe("SaikuroProvider.namespace", () => {
  it("exposes the namespace passed to the constructor", () => {
    const p = new SaikuroProvider("math");
    expect(p.namespace).toBe("math");
  });
});

describe("SaikuroProvider.register", () => {
  it("is chainable", () => {
    const p = new SaikuroProvider("math");
    const returned = p.register("add", () => 0).register("sub", () => 0);
    expect(returned).toBe(p);
  });
});

//  dispatch() :  sync handler

describe("SaikuroProvider.dispatch sync handler", () => {
  it("sends ok:true with the handler's return value", async () => {
    const [senderTransport, receiverTransport] = InMemoryTransport.pair();
    const provider = new SaikuroProvider("math");
    provider.register(
      "add",
      (a: unknown, b: unknown) => (a as number) + (b as number),
    );

    const responses = collectMessages(receiverTransport);
    await provider.dispatch(
      makeEnv("call", "math.add", [3, 4]),
      senderTransport,
    );

    expect(responses).toHaveLength(1);
    expect(responses[0]).toMatchObject({ id: "test-id", ok: true, result: 7 });
  });

  it("uses the last segment of target as the function name", async () => {
    const [senderTransport, receiverTransport] = InMemoryTransport.pair();
    const provider = new SaikuroProvider("math");
    provider.register(
      "mul",
      (a: unknown, b: unknown) => (a as number) * (b as number),
    );

    const responses = collectMessages(receiverTransport);
    await provider.dispatch(
      makeEnv("call", "math.mul", [3, 5]),
      senderTransport,
    );

    expect(responses[0]).toMatchObject({ ok: true, result: 15 });
  });

  it("returns null result when handler returns null", async () => {
    const [senderTransport, receiverTransport] = InMemoryTransport.pair();
    const provider = new SaikuroProvider("test");
    provider.register("nil", () => null);

    const responses = collectMessages(receiverTransport);
    await provider.dispatch(makeEnv("call", "test.nil", []), senderTransport);

    expect(responses[0]).toMatchObject({ ok: true, result: null });
  });
});

//  dispatch() :  async handler

describe("SaikuroProvider.dispatch async handler", () => {
  it("sends ok:true with the resolved value", async () => {
    const [senderTransport, receiverTransport] = InMemoryTransport.pair();
    const provider = new SaikuroProvider("math");
    provider.register(
      "asyncAdd",
      async (a: unknown, b: unknown) => (a as number) + (b as number),
    );

    const responses = collectMessages(receiverTransport);
    await provider.dispatch(
      makeEnv("call", "math.asyncAdd", [10, 20]),
      senderTransport,
    );

    expect(responses[0]).toMatchObject({ ok: true, result: 30 });
  });
});

//  dispatch() :  async generator (stream)

describe("SaikuroProvider.dispatch stream handler", () => {
  it("sends seq-numbered items then a stream_control:end sentinel", async () => {
    const [senderTransport, receiverTransport] = InMemoryTransport.pair();
    const provider = new SaikuroProvider("test");
    provider.register("count", async function* () {
      yield "a";
      yield "b";
      yield "c";
    });

    const responses = collectMessages(receiverTransport);
    await provider.dispatch(
      makeEnv("stream", "test.count", []),
      senderTransport,
    );

    // 3 data frames + 1 end sentinel
    expect(responses).toHaveLength(4);
    expect(responses[0]).toMatchObject({ ok: true, result: "a", seq: 0 });
    expect(responses[1]).toMatchObject({ ok: true, result: "b", seq: 1 });
    expect(responses[2]).toMatchObject({ ok: true, result: "c", seq: 2 });
    expect(responses[3]).toMatchObject({ ok: true, stream_control: "end" });
  });

  it("sends zero data frames and just an end sentinel for an empty generator", async () => {
    const [senderTransport, receiverTransport] = InMemoryTransport.pair();
    const provider = new SaikuroProvider("test");
    provider.register("empty", async function* () {
      /* nothing */
    });

    const responses = collectMessages(receiverTransport);
    await provider.dispatch(
      makeEnv("stream", "test.empty", []),
      senderTransport,
    );

    expect(responses).toHaveLength(1);
    expect(responses[0]).toMatchObject({ ok: true, stream_control: "end" });
  });

  it("emits error then abort when generator throws mid-stream", async () => {
    const [senderTransport, receiverTransport] = InMemoryTransport.pair();
    const provider = new SaikuroProvider("test");
    provider.register("failAfterOne", async function* () {
      yield 42;
      throw new Error("boom mid-stream");
    });

    const responses = collectMessages(receiverTransport);
    await provider.dispatch(
      makeEnv("stream", "test.failAfterOne", []),
      senderTransport,
    );

    // First frame is the data item, second is the error, third is abort.
    expect(responses).toHaveLength(3);
    expect(responses[0]).toMatchObject({ ok: true, result: 42 });
    expect(responses[1]).toMatchObject({ ok: false });
    expect(
      (responses[1]?.["error"] as Record<string, unknown>)?.["message"],
    ).toContain("boom mid-stream");
    expect(responses[2]).toMatchObject({ ok: false, stream_control: "abort" });
  });
});

//  dispatch() :  error cases

describe("SaikuroProvider.dispatch error paths", () => {
  it("sends FunctionNotFound when the target is not registered", async () => {
    const [senderTransport, receiverTransport] = InMemoryTransport.pair();
    const provider = new SaikuroProvider("test");

    const responses = collectMessages(receiverTransport);
    await provider.dispatch(
      makeEnv("call", "test.missing", []),
      senderTransport,
    );

    expect(responses[0]).toMatchObject({ ok: false });
    expect((responses[0]?.["error"] as Record<string, unknown>)?.["code"]).toBe(
      "FunctionNotFound",
    );
  });

  it("sends ProviderError when the handler throws a plain Error", async () => {
    const [senderTransport, receiverTransport] = InMemoryTransport.pair();
    const provider = new SaikuroProvider("test");
    provider.register("fail", () => {
      throw new Error("handler blew up");
    });

    const responses = collectMessages(receiverTransport);
    await provider.dispatch(makeEnv("call", "test.fail", []), senderTransport);

    expect(responses[0]).toMatchObject({ ok: false });
    const err = responses[0]?.["error"] as Record<string, unknown>;
    expect(err?.["code"]).toBe("ProviderError");
    expect(err?.["message"]).toContain("handler blew up");
  });

  it("preserves code and message when handler throws a SaikuroError", async () => {
    const [senderTransport, receiverTransport] = InMemoryTransport.pair();
    const provider = new SaikuroProvider("test");
    provider.register("deny", () => {
      throw new SaikuroError({
        code: "CapabilityDenied",
        message: "access denied",
      });
    });

    const responses = collectMessages(receiverTransport);
    await provider.dispatch(makeEnv("call", "test.deny", []), senderTransport);

    expect(responses[0]).toMatchObject({ ok: false });
    const err = responses[0]?.["error"] as Record<string, unknown>;
    expect(err?.["code"]).toBe("CapabilityDenied");
    expect(err?.["message"]).toContain("access denied");
  });
});

//  schemaObject()

describe("SaikuroProvider.schemaObject", () => {
  it("returns a schema with version 1", () => {
    const p = new SaikuroProvider("math");
    expect(p.schemaObject().version).toBe(1);
  });

  it("includes the namespace in the schema", () => {
    const p = new SaikuroProvider("math");
    p.register("add", () => 0);
    const schema = p.schemaObject();
    expect(schema.namespaces["math"]).toBeDefined();
  });

  it("includes each registered function", () => {
    const p = new SaikuroProvider("math");
    p.register("add", () => 0);
    p.register("sub", () => 0);
    const ns = p.schemaObject().namespaces["math"];
    expect(ns?.functions["add"]).toBeDefined();
    expect(ns?.functions["sub"]).toBeDefined();
  });

  it("stores doc when provided", () => {
    const p = new SaikuroProvider("math");
    p.register("add", () => 0, { doc: "adds two numbers" });
    const fn = p.schemaObject().namespaces["math"]?.functions["add"];
    expect(fn?.doc).toBe("adds two numbers");
  });

  it("stores capabilities when provided", () => {
    const p = new SaikuroProvider("math");
    p.register("secret", () => 0, { capabilities: ["admin"] });
    const fn = p.schemaObject().namespaces["math"]?.functions["secret"];
    expect(fn?.capabilities).toEqual(["admin"]);
  });

  it("returns empty functions object when nothing is registered", () => {
    const p = new SaikuroProvider("empty");
    const ns = p.schemaObject().namespaces["empty"];
    expect(ns?.functions).toEqual({});
  });
});

//  decorator()

describe("SaikuroProvider.decorator", () => {
  it("registers the decorated function", async () => {
    const [senderTransport, receiverTransport] = InMemoryTransport.pair();
    const provider = new SaikuroProvider("test");

    const decorated = provider.decorator("greet")(
      (name: unknown) => `hi ${name as string}`,
    );

    const responses = collectMessages(receiverTransport);
    await provider.dispatch(
      makeEnv("call", "test.greet", ["alice"]),
      senderTransport,
    );

    expect(responses[0]).toMatchObject({ ok: true, result: "hi alice" });
    // The decorator returns the original function unchanged.
    expect(typeof decorated).toBe("function");
  });
});

//  schemaObject() :  type descriptors

describe("SaikuroProvider.schemaObject type descriptors", () => {
  it("emits args descriptors when provided via register()", () => {
    const p = new SaikuroProvider("math");
    p.register("add", () => 0, {
      args: [
        { name: "a", type: t.i32() },
        { name: "b", type: t.i32() },
      ],
    });
    const fn = p.schemaObject().namespaces["math"]?.functions["add"];
    expect(fn?.args).toHaveLength(2);
    expect(fn?.args?.[0]).toMatchObject({
      name: "a",
      type: { kind: "primitive", type: "i32" },
    });
    expect(fn?.args?.[1]).toMatchObject({
      name: "b",
      type: { kind: "primitive", type: "i32" },
    });
  });

  it("emits returns descriptor when provided via register()", () => {
    const p = new SaikuroProvider("math");
    p.register("add", () => 0, { returns: t.i64() });
    const fn = p.schemaObject().namespaces["math"]?.functions["add"];
    expect(fn?.returns).toEqual({ kind: "primitive", type: "i64" });
  });

  it("falls back to empty args array when args not provided", () => {
    const p = new SaikuroProvider("math");
    p.register("add", () => 0);
    const fn = p.schemaObject().namespaces["math"]?.functions["add"];
    expect(fn?.args).toEqual([]);
  });

  it("falls back to any returns when returns not provided", () => {
    const p = new SaikuroProvider("math");
    p.register("add", () => 0);
    const fn = p.schemaObject().namespaces["math"]?.functions["add"];
    expect(fn?.returns).toEqual({ kind: "primitive", type: "any" });
  });

  it("emits visibility when provided", () => {
    const p = new SaikuroProvider("svc");
    p.register("internal", () => 0, { visibility: "internal" });
    const fn = p.schemaObject().namespaces["svc"]?.functions["internal"];
    expect(fn?.visibility).toBe("internal");
  });

  it("defaults visibility to public when not provided", () => {
    const p = new SaikuroProvider("svc");
    p.register("pub", () => 0);
    const fn = p.schemaObject().namespaces["svc"]?.functions["pub"];
    expect(fn?.visibility).toBe("public");
  });

  it("emits idempotent flag when provided", () => {
    const p = new SaikuroProvider("svc");
    p.register("fetch", () => 0, { idempotent: true });
    const fn = p.schemaObject().namespaces["svc"]?.functions["fetch"];
    expect(fn?.idempotent).toBe(true);
  });

  it("does not emit idempotent when not provided", () => {
    const p = new SaikuroProvider("svc");
    p.register("fetch", () => 0);
    const fn = p.schemaObject().namespaces["svc"]?.functions["fetch"];
    expect(fn?.idempotent).toBeUndefined();
  });

  it("emits complex nested type descriptors correctly", () => {
    const p = new SaikuroProvider("svc");
    p.register("search", () => 0, {
      args: [{ name: "query", type: t.optional(t.string()) }],
      returns: t.list(t.named("SearchResult")),
    });
    const fn = p.schemaObject().namespaces["svc"]?.functions["search"];
    expect(fn?.args?.[0]).toMatchObject({
      name: "query",
      type: { kind: "optional", inner: { kind: "primitive", type: "string" } },
    });
    expect(fn?.returns).toEqual({
      kind: "list",
      item: { kind: "named", name: "SearchResult" },
    });
  });

  it("emits stream return type", () => {
    const p = new SaikuroProvider("svc");
    p.register(
      "events",
      async function* () {
        yield 1;
      },
      { returns: t.stream(t.string()) },
    );
    const fn = p.schemaObject().namespaces["svc"]?.functions["events"];
    expect(fn?.returns).toEqual({
      kind: "stream",
      item: { kind: "primitive", type: "string" },
    });
  });

  it("emits channel type", () => {
    const p = new SaikuroProvider("svc");
    p.register("chat", () => 0, { returns: t.channel(t.string(), t.string()) });
    const fn = p.schemaObject().namespaces["svc"]?.functions["chat"];
    expect(fn?.returns).toEqual({
      kind: "channel",
      send: { kind: "primitive", type: "string" },
      recv: { kind: "primitive", type: "string" },
    });
  });

  it("emits arg optional flag", () => {
    const p = new SaikuroProvider("svc");
    p.register("greet", () => 0, {
      args: [{ name: "suffix", type: t.string(), optional: true }],
    });
    const fn = p.schemaObject().namespaces["svc"]?.functions["greet"];
    expect(fn?.args?.[0]).toMatchObject({ optional: true });
  });

  it("decorator forwards args and returns options", () => {
    const p = new SaikuroProvider("svc");
    p.decorator("mul", {
      args: [{ name: "x", type: t.f64() }],
      returns: t.f64(),
    })(() => 0);
    const fn = p.schemaObject().namespaces["svc"]?.functions["mul"];
    expect(fn?.args?.[0]).toMatchObject({
      name: "x",
      type: { kind: "primitive", type: "f64" },
    });
    expect(fn?.returns).toEqual({ kind: "primitive", type: "f64" });
  });
});
