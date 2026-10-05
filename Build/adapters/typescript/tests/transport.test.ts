/**
 * Tests for InMemoryTransport
 *
 */

import { describe, it, expect, vi } from "vitest";
import { InMemoryTransport } from "../src/transport";
import { MAX_BUFFERED_MESSAGES } from "../src/transport/base";

describe("InMemoryTransport.pair", () => {
  it("returns two transport instances", () => {
    const [a, b] = InMemoryTransport.pair();
    expect(a).toBeInstanceOf(InMemoryTransport);
    expect(b).toBeInstanceOf(InMemoryTransport);
  });

  it("returns distinct instances", () => {
    const [a, b] = InMemoryTransport.pair();
    expect(a).not.toBe(b);
  });
});

describe("InMemoryTransport.connect", () => {
  it("resolves immediately", async () => {
    const [a] = InMemoryTransport.pair();
    await expect(a.connect()).resolves.toBeUndefined();
  });
});

describe("InMemoryTransport.send / onMessage", () => {
  it("delivers a message to the peer's onMessage handler", async () => {
    const [a, b] = InMemoryTransport.pair();
    const received: Record<string, unknown>[] = [];
    b.onMessage((msg) => received.push(msg));
    await a.send({ hello: "world" });
    expect(received).toHaveLength(1);
    expect(received[0]).toEqual({ hello: "world" });
  });

  it("delivers to multiple onMessage handlers in registration order", async () => {
    const [a, b] = InMemoryTransport.pair();
    const order: number[] = [];
    b.onMessage(() => order.push(1));
    b.onMessage(() => order.push(2));
    await a.send({ x: 1 });
    expect(order).toEqual([1, 2]);
  });

  it("send in the other direction also works", async () => {
    const [a, b] = InMemoryTransport.pair();
    const received: Record<string, unknown>[] = [];
    a.onMessage((msg) => received.push(msg));
    await b.send({ reply: true });
    expect(received).toEqual([{ reply: true }]);
  });

  it("delivers multiple sequential messages in order", async () => {
    const [a, b] = InMemoryTransport.pair();
    const received: number[] = [];
    b.onMessage((msg) => received.push(msg["n"] as number));
    await a.send({ n: 1 });
    await a.send({ n: 2 });
    await a.send({ n: 3 });
    expect(received).toEqual([1, 2, 3]);
  });
});

describe("InMemoryTransport.offMessage", () => {
  it("removes a registered handler so it no longer fires", async () => {
    const [a, b] = InMemoryTransport.pair();
    const handler = vi.fn();
    b.onMessage(handler);
    b.offMessage(handler);
    await a.send({ x: 1 });
    expect(handler).not.toHaveBeenCalled();
  });

  it("is a no-op for a handler that was never registered", () => {
    const [, b] = InMemoryTransport.pair();
    // Should not throw.
    expect(() => b.offMessage(() => {})).not.toThrow();
  });

  it("only removes the specific handler, leaving others intact", async () => {
    const [a, b] = InMemoryTransport.pair();
    const keep = vi.fn();
    const remove = vi.fn();
    b.onMessage(keep);
    b.onMessage(remove);
    b.offMessage(remove);
    await a.send({ y: 2 });
    expect(keep).toHaveBeenCalledOnce();
    expect(remove).not.toHaveBeenCalled();
  });

  it("handler may call offMessage on itself without causing re-entrant delivery", async () => {
    // Regression: live-Set iteration caused re-entrant infinite loops when a
    // handler deregistered and re-registered itself during delivery.
    const [a, b] = InMemoryTransport.pair();
    let callCount = 0;

    function selfDeregister(raw: Record<string, unknown>): void {
      b.offMessage(selfDeregister);
      callCount++;
      // Re-register for the next message :  should NOT re-fire for this delivery.
      b.onMessage(selfDeregister);
      void raw; // suppress lint
    }

    b.onMessage(selfDeregister);
    await a.send({ n: 1 }); // fires once
    await a.send({ n: 2 }); // fires once more
    expect(callCount).toBe(2);
  });
});

describe("BaseTransport buffering before onMessage", () => {
  it("replays a message received before onMessage registered", async () => {
    const [a, b] = InMemoryTransport.pair();
    await b.send({ early: true });

    const received: Record<string, unknown>[] = [];
    a.onMessage((msg) => received.push(msg));

    expect(received).toEqual([{ early: true }]);
  });

  it("replays in arrival order", async () => {
    const [a, b] = InMemoryTransport.pair();
    await b.send({ n: 1 });
    await b.send({ n: 2 });
    await b.send({ n: 3 });

    const received: Record<string, unknown>[] = [];
    a.onMessage((msg) => received.push(msg));

    expect(received).toEqual([{ n: 1 }, { n: 2 }, { n: 3 }]);
  });

  it("replays only once and then delivers live", async () => {
    const [a, b] = InMemoryTransport.pair();
    await b.send({ buffered: true });

    const received: Record<string, unknown>[] = [];
    a.onMessage((msg) => received.push(msg));
    await b.send({ live: true });
    await vi.waitFor(() => expect(received).toHaveLength(2));

    expect(received).toEqual([{ buffered: true }, { live: true }]);
  });

  it("gives an early message to the handler that registers first", async () => {
    const [a, b] = InMemoryTransport.pair();
    await b.send({ early: true });

    const first: Record<string, unknown>[] = [];
    const second: Record<string, unknown>[] = [];
    a.onMessage((msg) => first.push(msg));
    a.onMessage((msg) => second.push(msg));

    expect(first).toEqual([{ early: true }]);
    expect(second).toEqual([]);
  });

  it("drops the newest message once the buffer is full", async () => {
    const [a, b] = InMemoryTransport.pair();
    for (let n = 0; n < MAX_BUFFERED_MESSAGES + 5; n += 1) {
      await b.send({ n });
    }

    const received: Record<string, unknown>[] = [];
    a.onMessage((msg) => received.push(msg));

    // The cap is honoured and the earliest frames are the ones kept.
    expect(received).toHaveLength(MAX_BUFFERED_MESSAGES);
    expect(received[0]).toEqual({ n: 0 });
  });

  it("replays reentrant messages after everything buffered earlier", async () => {
    const [a, b] = InMemoryTransport.pair();
    await b.send({ step: 1 });
    await b.send({ step: 2 });

    const received: Record<string, unknown>[] = [];
    a.onMessage((msg) => {
      received.push(msg);
      if (msg.step !== undefined) void b.send({ reply: msg.step });
    });

    // The echoed replies arrived last, so they must not jump the queue.
    expect(received).toEqual([
      { step: 1 },
      { step: 2 },
      { reply: 1 },
      { reply: 2 },
    ]);
  });
});

describe("InMemoryTransport.recv", () => {
  it("returns null (push-only transport)", async () => {
    const [a] = InMemoryTransport.pair();
    await expect(a.recv()).resolves.toBeNull();
  });
});

describe("InMemoryTransport.close", () => {
  it("throws when sending after close", async () => {
    const [a] = InMemoryTransport.pair();
    await a.close();
    await expect(a.send({ x: 1 })).rejects.toThrow("transport is closed");
  });

  it("fires onClose handler on the transport that called close()", async () => {
    const [a] = InMemoryTransport.pair();
    const handler = vi.fn();
    a.onClose(handler);
    await a.close();
    expect(handler).toHaveBeenCalledOnce();
  });

  it("fires onClose handler on the peer as well", async () => {
    const [a, b] = InMemoryTransport.pair();
    const peerClose = vi.fn();
    b.onClose(peerClose);
    await a.close();
    expect(peerClose).toHaveBeenCalledOnce();
  });

  it("close() resolves without error", async () => {
    const [a] = InMemoryTransport.pair();
    await expect(a.close()).resolves.toBeUndefined();
  });
});
