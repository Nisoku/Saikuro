/**
 * Tests for SaikuroClient.
 *
 * Uses InMemoryTransport.pair() to wire a real SaikuroProvider on one side
 * and the client under test on the other, so no mocking of the wire protocol
 * is needed (the obviously better way to do it)
 */

import { describe, it, expect, vi } from "vitest";
import { InMemoryTransport } from "../src/transport";
import { SaikuroClient } from "../src/client";
import { SaikuroProvider } from "../src/provider";
import { SaikuroError, TransportError } from "../src/error";
import type { ResourceHandle } from "../src/envelope";

//  Helper: wire a client + provider

interface Harness {
  client: SaikuroClient;
  provider: SaikuroProvider;
  /** Teardown :  close the client (which signals the provider side). */
  teardown: () => Promise<void>;
}

/**
 * Wire a SaikuroClient and SaikuroProvider together over an InMemoryTransport
 * pair, bypassing the announce handshake (not needed for unit tests).
 *
 * The provider dispatch loop is started manually so there is no 5-second
 * announce timeout blocking the tests.
 */
async function makeHarness(namespace = "test"): Promise<Harness> {
  const [clientTransport, providerTransport] = InMemoryTransport.pair();
  const provider = new SaikuroProvider(namespace);
  const client = SaikuroClient.fromTransport(clientTransport);
  await client.open();

  // Manually wire up the provider's dispatch loop without announcement.
  // Messages arriving on providerTransport are dispatched; providerTransport
  // sends responses back to clientTransport.
  const servePromise = new Promise<void>((resolve) => {
    providerTransport.onClose(() => resolve());
    providerTransport.onMessage((raw) => {
      // Build a minimal Envelope from the raw map.
      const envelope = {
        version: (raw["version"] as number) ?? 1,
        type: raw["type"] as
          | "call"
          | "cast"
          | "stream"
          | "channel"
          | "batch"
          | "resource"
          | "log"
          | "announce",
        id: raw["id"] as string,
        target: raw["target"] as string,
        args: (raw["args"] as unknown[]) ?? [],
      };
      void provider.dispatch(envelope, providerTransport);
    });
  });

  const teardown = async (): Promise<void> => {
    await client.close();
    await servePromise.catch(() => {});
  };

  return { client, provider, teardown };
}

//  Lifecycle

describe("SaikuroClient lifecycle", () => {
  it("fromTransport + open makes connected   true", async () => {
    const [t] = InMemoryTransport.pair();
    const client = SaikuroClient.fromTransport(t);
    expect(client.connected).toBe(false);
    await client.open();
    expect(client.connected).toBe(true);
    await client.close();
  });

  it("close() makes connected   false", async () => {
    const [t] = InMemoryTransport.pair();
    const client = SaikuroClient.fromTransport(t);
    await client.open();
    await client.close();
    expect(client.connected).toBe(false);
  });
});

//  call()

describe("SaikuroClient.call", () => {
  it("returns the result from a synchronous handler", async () => {
    const h = await makeHarness();
    h.provider.register(
      "add",
      (a: unknown, b: unknown) => (a as number) + (b as number),
    );
    const result = await h.client.call("test.add", [3, 4]);
    expect(result).toBe(7);
    await h.teardown();
  });

  it("returns the result from an async handler", async () => {
    const h = await makeHarness();
    h.provider.register(
      "greet",
      async (name: unknown) => `Hello, ${name as string}`,
    );
    const result = await h.client.call("test.greet", ["world"]);
    expect(result).toBe("Hello, world");
    await h.teardown();
  });

  it("throws SaikuroError when the provider handler throws", async () => {
    const h = await makeHarness();
    h.provider.register("boom", () => {
      throw new Error("exploded");
    });
    await expect(h.client.call("test.boom", [])).rejects.toBeInstanceOf(
      SaikuroError,
    );
    await h.teardown();
  });

  it("throws SaikuroError with the handler's error message", async () => {
    const h = await makeHarness();
    h.provider.register("boom", () => {
      throw new Error("exploded");
    });
    const err = await h.client.call("test.boom", []).catch((e: unknown) => e);
    expect((err as SaikuroError).message).toContain("exploded");
    await h.teardown();
  });

  it("throws when calling a function that is not registered", async () => {
    const h = await makeHarness();
    await expect(h.client.call("test.missing", [])).rejects.toBeInstanceOf(
      SaikuroError,
    );
    await h.teardown();
  });

  it("passes args correctly (null, string, array)", async () => {
    const h = await makeHarness();
    h.provider.register("echo", (...args: unknown[]) => args);
    const result = await h.client.call("test.echo", [null, "hi", [1, 2]]);
    expect(result).toEqual([null, "hi", [1, 2]]);
    await h.teardown();
  });

  it("returns null result", async () => {
    const h = await makeHarness();
    h.provider.register("nothing", () => null);
    const result = await h.client.call("test.nothing", []);
    expect(result).toBeNull();
    await h.teardown();
  });
});

//  call() with timeout

describe("SaikuroClient.call timeout", () => {
  it("rejects with a timeout message when the handler is too slow", async () => {
    const [clientTransport] = InMemoryTransport.pair();
    // No provider :  client will never receive a response.
    const client = SaikuroClient.fromTransport(clientTransport);
    await client.open();

    const err = await client
      .call("nowhere.fn", [], { timeoutMs: 20 })
      .catch((e: unknown) => e);

    expect(err).toBeInstanceOf(Error);
    expect((err as Error).message).toContain("timed out");
    await client.close();
  });

  it("defaultTimeoutMs is used when per-call timeoutMs is not set", async () => {
    const [clientTransport] = InMemoryTransport.pair();
    const client = SaikuroClient.fromTransport(clientTransport, {
      defaultTimeoutMs: 20,
    });
    await client.open();

    const err = await client.call("nowhere.fn", []).catch((e: unknown) => e);
    expect((err as Error).message).toContain("timed out");
    await client.close();
  });
});

//  cast()

describe("SaikuroClient.cast", () => {
  it("resolves without error even when no provider is listening", async () => {
    const [clientTransport] = InMemoryTransport.pair();
    const client = SaikuroClient.fromTransport(clientTransport);
    await client.open();
    await expect(
      client.cast("events.fire", [{ type: "click" }]),
    ).resolves.toBeUndefined();
    await client.close();
  });

  it("delivers the envelope to the provider transport", async () => {
    const h = await makeHarness();
    const received: unknown[] = [];
    h.provider.register("track", (...args: unknown[]) => {
      received.push(args[0]);
    });
    await h.client.cast("test.track", [{ ev: "view" }]);
    // Give the event loop a tick to dispatch.
    await new Promise((r) => setTimeout(r, 0));
    expect(received).toHaveLength(1);
    expect(received[0]).toEqual({ ev: "view" });
    await h.teardown();
  });
});

//  resource()

describe("SaikuroClient.resource", () => {
  it("decodes a ResourceHandle from a valid provider response", async () => {
    const h = await makeHarness();
    const handle: ResourceHandle = {
      id: "res-42",
      mime_type: "text/plain",
      size: 100,
      uri: "saikuro://res/res-42",
    };
    h.provider.register("open", () => handle);
    const result = await h.client.resource("test.open", []);
    expect(result).toEqual(handle);
    await h.teardown();
  });

  it("throws SaikuroError when the provider returns a non-handle value", async () => {
    const h = await makeHarness();
    h.provider.register("open", () => "not-a-handle");
    await expect(h.client.resource("test.open", [])).rejects.toBeInstanceOf(
      SaikuroError,
    );
    await h.teardown();
  });

  it("throws SaikuroError when the provider returns null", async () => {
    const h = await makeHarness();
    h.provider.register("open", () => null);
    await expect(h.client.resource("test.open", [])).rejects.toBeInstanceOf(
      SaikuroError,
    );
    await h.teardown();
  });

  it("throws SaikuroError when the provider errors", async () => {
    const h = await makeHarness();
    h.provider.register("open", () => {
      throw new Error("file not found");
    });
    await expect(h.client.resource("test.open", [])).rejects.toBeInstanceOf(
      SaikuroError,
    );
    await h.teardown();
  });
});

//  stream()

describe("SaikuroClient.stream", () => {
  it("yields all items from an async generator and then completes", async () => {
    const h = await makeHarness();
    h.provider.register("count", async function* () {
      yield 1;
      yield 2;
      yield 3;
    });

    const stream = await h.client.stream<number>("test.count", []);
    const items: number[] = [];
    for await (const item of stream) {
      items.push(item);
    }
    expect(items).toEqual([1, 2, 3]);
    await h.teardown();
  });

  it("throws SaikuroError when the generator throws mid-stream", async () => {
    const h = await makeHarness();
    h.provider.register("failStream", async function* () {
      yield 10;
      throw new Error("mid-stream failure");
    });

    const stream = await h.client.stream<number>("test.failStream", []);
    const items: number[] = [];
    let caughtErr: unknown;
    try {
      for await (const item of stream) {
        items.push(item);
      }
    } catch (e) {
      caughtErr = e;
    }

    expect(items).toEqual([10]);
    expect(caughtErr).toBeInstanceOf(SaikuroError);
    await h.teardown();
  });

  it("yields zero items for an immediately-returning generator", async () => {
    const h = await makeHarness();
    h.provider.register("empty", async function* () {
      // nothing
    });

    const stream = await h.client.stream<number>("test.empty", []);
    const items: number[] = [];
    for await (const item of stream) {
      items.push(item);
    }
    expect(items).toHaveLength(0);
    await h.teardown();
  });
});

//  channel()

describe("SaikuroClient.channel", () => {
  it("opens a channel with a unique invocationId", async () => {
    const h = await makeHarness();
    h.provider.register("chat", async function* () {
      yield "hello from provider";
    });

    const ch = await h.client.channel<string, string>("test.chat", []);
    expect(typeof ch.invocationId).toBe("string");
    expect(ch.invocationId.length).toBeGreaterThan(0);
    // Consume the channel to completion.
    for await (const _ of ch) {
      /* drain */
    }
    await h.teardown();
  });

  it("receives items yielded by the provider generator", async () => {
    const h = await makeHarness();
    h.provider.register("echo", async function* () {
      yield "a";
      yield "b";
    });

    const ch = await h.client.channel<string, string>("test.echo", []);
    const received: string[] = [];
    for await (const item of ch) {
      received.push(item);
    }
    expect(received).toEqual(["a", "b"]);
    await h.teardown();
  });
});

//  close() tears down pending calls

describe("SaikuroClient.close teardown", () => {
  it("rejects a pending call with TransportError when client is closed", async () => {
    // No provider :  client will never receive a response.
    const [clientTransport] = InMemoryTransport.pair();
    const client = SaikuroClient.fromTransport(clientTransport);
    await client.open();

    const callPromise = client.call("nowhere.fn", []);
    // Close in the background :  should reject the pending call.
    await client.close();

    const err = await callPromise.catch((e: unknown) => e);
    expect(err).toBeInstanceOf(TransportError);
  });
});

//  concurrent calls

describe("SaikuroClient concurrent calls", () => {
  it("resolves all concurrent calls independently", async () => {
    const h = await makeHarness();
    h.provider.register("double", (n: unknown) => (n as number) * 2);

    const results = await Promise.all([
      h.client.call("test.double", [1]),
      h.client.call("test.double", [2]),
      h.client.call("test.double", [3]),
      h.client.call("test.double", [4]),
    ]);
    expect(results.sort()).toEqual([2, 4, 6, 8]);
    await h.teardown();
  });
});

//  log()

describe("SaikuroClient.log", () => {
  it("resolves without error (fire-and-forget)", async () => {
    const [clientTransport] = InMemoryTransport.pair();
    const client = SaikuroClient.fromTransport(clientTransport);
    await client.open();
    await expect(
      client.log("info", "test", "hello", { k: "v" }),
    ).resolves.toBeUndefined();
    await client.close();
  });

  it("sends a log envelope that reaches the peer transport", async () => {
    const [clientTransport, providerTransport] = InMemoryTransport.pair();
    const client = SaikuroClient.fromTransport(clientTransport);
    await client.open();

    const messages: Record<string, unknown>[] = [];
    providerTransport.onMessage((m) => messages.push(m));

    await client.log("warn", "my.logger", "something happened", { detail: 42 });

    expect(messages).toHaveLength(1);
    expect(messages[0]?.["type"]).toBe("log");
    expect(messages[0]?.["target"]).toBe("$log");
    const logRecord = (messages[0]?.["args"] as unknown[])?.[0] as Record<
      string,
      unknown
    >;
    expect(logRecord?.["level"]).toBe("warn");
    expect(logRecord?.["msg"]).toBe("something happened");

    await client.close();
  });
});

//  Spy on vi.fn

describe("SaikuroClient handler invocation count", () => {
  it("handler is called exactly once per call", async () => {
    const h = await makeHarness();
    const handler = vi.fn(() => 99);
    h.provider.register("fn", handler);
    await h.client.call("test.fn", []);
    expect(handler).toHaveBeenCalledOnce();
    await h.teardown();
  });
});

//  batch()

/**
 * Minimal batch server: listens on `transport`, receives a batch envelope,
 * dispatches each batch_item to the matching handler in `handlers` (keyed by
 * the last segment of the target), and sends back a single response whose
 * `result` is the ordered array of per-item results (null for missing/failed
 * handlers).
 *
 * Returns a cleanup function that deregisters the server.
 */
function makeBatchServer(
  transport: InMemoryTransport,
  handlers: Record<string, (...args: unknown[]) => unknown>,
): () => void {
  const handleBatch = (raw: Record<string, unknown>): void => {
    const batchId = raw["id"] as string;
    const items = (raw["batch_items"] ?? []) as Array<Record<string, unknown>>;

    const results: unknown[] = items.map((item) => {
      const target = item["target"] as string;
      const args = (item["args"] ?? []) as unknown[];
      const fnName = target.includes(".")
        ? target.slice(target.lastIndexOf(".") + 1)
        : target;
      const handler = handlers[fnName];
      if (handler === undefined) return null;
      try {
        return handler(...args);
      } catch {
        return null;
      }
    });

    void transport.send({ id: batchId, ok: true, result: results });
  };

  transport.onMessage(handleBatch);
  return () => transport.offMessage(handleBatch);
}

describe("SaikuroClient.batch", () => {
  it("returns ordered results for multiple calls", async () => {
    const [clientTransport, serverTransport] = InMemoryTransport.pair();
    const client = SaikuroClient.fromTransport(clientTransport);
    await client.open();

    const stop = makeBatchServer(serverTransport, {
      add: (a: unknown, b: unknown) => (a as number) + (b as number),
      mul: (a: unknown, b: unknown) => (a as number) * (b as number),
    });

    const results = await client.batch([
      { target: "math.add", args: [2, 3] },
      { target: "math.mul", args: [4, 5] },
    ]);

    expect(results).toEqual([5, 20]);
    stop();
    await client.close();
  });

  it("handles a single-item batch", async () => {
    const [clientTransport, serverTransport] = InMemoryTransport.pair();
    const client = SaikuroClient.fromTransport(clientTransport);
    await client.open();

    const stop = makeBatchServer(serverTransport, {
      echo: (v: unknown) => v,
    });

    const results = await client.batch([
      { target: "svc.echo", args: ["hello"] },
    ]);

    expect(results).toEqual(["hello"]);
    stop();
    await client.close();
  });

  it("propagates capability to batch items", async () => {
    const [clientTransport, serverTransport] = InMemoryTransport.pair();
    const client = SaikuroClient.fromTransport(clientTransport);
    await client.open();

    const captured: Record<string, unknown>[] = [];
    serverTransport.onMessage((raw) => {
      captured.push(raw);
      const batchId = raw["id"] as string;
      void serverTransport.send({ id: batchId, ok: true, result: [42] });
    });

    const results = await client.batch([
      { target: "svc.fn", args: [1], capability: "my.cap" },
    ]);

    expect(results).toEqual([42]);
    expect(captured).toHaveLength(1);
    const items = (captured[0]?.["batch_items"] ?? []) as Array<
      Record<string, unknown>
    >;
    expect(items).toHaveLength(1);
    expect(items[0]?.["capability"]).toBe("my.cap");
    await client.close();
  });

  it("returns null for failed/missing batch items", async () => {
    const [clientTransport, serverTransport] = InMemoryTransport.pair();
    const client = SaikuroClient.fromTransport(clientTransport);
    await client.open();

    // "ok" is registered; "missing" is not → null result.
    const stop = makeBatchServer(serverTransport, {
      ok: () => "fine",
    });

    const results = await client.batch([
      { target: "svc.ok", args: [] },
      { target: "svc.missing", args: [] },
    ]);

    expect(results[0]).toBe("fine");
    expect(results[1]).toBeNull();
    stop();
    await client.close();
  });

  it("throws SaikuroError when the batch envelope is rejected", async () => {
    const [clientTransport, serverTransport] = InMemoryTransport.pair();
    const client = SaikuroClient.fromTransport(clientTransport);
    await client.open();

    serverTransport.onMessage((raw) => {
      const id = raw["id"] as string;
      void serverTransport.send({
        id,
        ok: false,
        error: { code: "MalformedEnvelope", message: "empty batch" },
      });
    });

    await expect(
      client.batch([{ target: "svc.fn", args: [] }]),
    ).rejects.toBeInstanceOf(SaikuroError);

    await client.close();
  });

  it("throws on timeout when no response arrives", async () => {
    const [clientTransport] = InMemoryTransport.pair();
    const client = SaikuroClient.fromTransport(clientTransport);
    await client.open();

    await expect(
      client.batch([{ target: "svc.fn", args: [] }], { timeoutMs: 20 }),
    ).rejects.toThrow("timed out");

    await client.close();
  });

  it("sends a batch-type envelope with batch_items on the wire", async () => {
    const [clientTransport, serverTransport] = InMemoryTransport.pair();
    const client = SaikuroClient.fromTransport(clientTransport);
    await client.open();

    const captured: Record<string, unknown>[] = [];
    serverTransport.onMessage((raw) => {
      captured.push(raw);
      const id = raw["id"] as string;
      void serverTransport.send({ id, ok: true, result: [null, null] });
    });

    await client.batch([
      { target: "ns.fn1", args: [1] },
      { target: "ns.fn2", args: [2] },
    ]);

    expect(captured).toHaveLength(1);
    expect(captured[0]?.["type"]).toBe("batch");
    const items = (captured[0]?.["batch_items"] ?? []) as Array<
      Record<string, unknown>
    >;
    expect(items).toHaveLength(2);
    expect(items[0]?.["target"]).toBe("ns.fn1");
    expect(items[1]?.["target"]).toBe("ns.fn2");
    await client.close();
  });
});
