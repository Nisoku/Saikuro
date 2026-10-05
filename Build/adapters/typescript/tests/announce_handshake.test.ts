/**
 * Tests for the provider/client schema-announce handshake.
 */

import { describe, it, expect, vi } from "vitest";
import { InMemoryTransport } from "../src/transport";
import { SaikuroClient } from "../src/client";
import { SaikuroProvider } from "../src/provider";
import { makeAnnounceEnvelope, makeSchemaObject } from "../src/envelope";
import { setLogSink, resetLogSink, type LogRecord } from "../src/logger";
import type { Transport } from "../src/transport";

/** Timeout for calls that must complete without waiting out the handshake. */
const PROMPT_MS = 2_000;

/**
 * Collect the acks a peer sends back, i.e. response frames that carry `ok`.
 * Announce and invocation frames travel the other way and are ignored.
 */
function collectAcks(transport: Transport): Array<Record<string, unknown>> {
  const acks: Array<Record<string, unknown>> = [];
  transport.onMessage((raw) => {
    if (raw["ok"] !== undefined) acks.push(raw);
  });
  return acks;
}

describe("schema announce handshake", () => {
  /**
   * Transport whose `recv` blocks once the inbox is empty
   */
  class GatedTransport extends InMemoryTransport {
    private readonly _waiters: Array<
      (frame: Record<string, unknown> | null) => void
    > = [];

    override async recv(): Promise<Record<string, unknown> | null> {
      const inbox = (this as unknown as { _inbox: unknown[] })._inbox;
      if (inbox.length > 0) return super.recv();
      return new Promise<Record<string, unknown> | null>((resolve) => {
        this._waiters.push(resolve);
      });
    }

    /** Hand `frame` to whichever `recv` is currently parked. */
    release(frame: Record<string, unknown>): void {
      this._waiters.shift()?.(frame);
    }
  }

  /** Pair a gated client-side transport with a peer that can send into it. */
  function gatedPair(): [GatedTransport, InMemoryTransport] {
    const gated = new GatedTransport();
    const peer = new InMemoryTransport();
    (gated as unknown as { _peer: unknown })._peer = peer;
    (peer as unknown as { _peer: unknown })._peer = gated;
    return [gated, peer];
  }
  it("client acks a schema announce with an ok response", async () => {
    const [clientTransport, providerTransport] = InMemoryTransport.pair();

    const client = SaikuroClient.fromTransport(clientTransport);
    await client.open();

    const acks = collectAcks(providerTransport);
    const announce = makeAnnounceEnvelope(makeSchemaObject("math", {}));
    // Sent from the provider side so it lands on the client's transport.
    await providerTransport.send(announce);

    expect(acks).toHaveLength(1);
    expect(acks[0]?.["ok"]).toBe(true);
    expect(acks[0]?.["id"]).toEqual(announce.id);
  });

  it("client acks an announce that was buffered before open", async () => {
    const [clientTransport, providerTransport] = InMemoryTransport.pair();

    const announce = makeAnnounceEnvelope(makeSchemaObject("math", {}));

    // Sent while the client is still closed.
    await providerTransport.send(announce);
    const acks = collectAcks(providerTransport);

    const client = SaikuroClient.fromTransport(clientTransport);
    await client.open();

    expect(acks).toHaveLength(1);
    expect(acks[0]?.["ok"]).toBe(true);
    expect(acks[0]?.["id"]).toEqual(announce.id);
  });

  it("serveOn completes the handshake and drops no invocation", async () => {
    const [clientTransport, providerTransport] = InMemoryTransport.pair();

    const provider = new SaikuroProvider("math");
    provider.register("add", (...args: unknown[]) => Number(args[0]) + 1);

    // The client is opened first so it can answer the announce.
    const client = SaikuroClient.fromTransport(clientTransport);
    await client.open();

    const servePromise = provider.serveOn(providerTransport);
    const sum = (await client.call("math.add", [41], {
      timeoutMs: PROMPT_MS,
    })) as number;

    expect(sum).toBe(42);

    await client.close();
    await servePromise;
  });

  it("ignores a malformed announce instead of acking an undefined id", async () => {
    const [clientTransport, providerTransport] = InMemoryTransport.pair();

    const client = SaikuroClient.fromTransport(clientTransport);
    await client.open();

    const records: LogRecord[] = [];
    setLogSink((record) => records.push(record));

    // An announce without a usable byte id must not be acked, and must not take
    // down the connection.
    await providerTransport.send({
      type: "announce",
      id: "not-bytes",
      schema: makeSchemaObject("math", {}),
    } as unknown as Record<string, unknown>);
    await vi.waitFor(() =>
      expect(records.some((r) => r.msg.includes("without a byte id"))).toBe(
        true,
      ),
    );
    resetLogSink();

    expect(client.connected).toBe(true);
    await client.close();
  });

  it("does not drop a frame that arrived before open", async () => {
    const [clientTransport, providerTransport] = InMemoryTransport.pair();

    // A response lands while the client is still closed: it stays in the
    // transport inbox and the pre-open drain has to hand it to the dispatcher
    // rather than swallow it.
    await providerTransport.send({
      id: new Uint8Array([9, 9]),
      ok: true,
      result: 1,
    });
    await providerTransport.send(
      makeAnnounceEnvelope(makeSchemaObject("math", {})),
    );

    const records: LogRecord[] = [];
    setLogSink((record) => records.push(record));

    const client = SaikuroClient.fromTransport(clientTransport);
    await client.open();
    await vi.waitFor(() =>
      expect(
        records.some(
          (r) =>
            r.msg.includes("no matching pending") &&
            JSON.stringify(r.state ?? {}).includes("0909"),
        ),
      ).toBe(true),
    );
    resetLogSink();

    await client.close();
  });

  it("acks once when an announce arrives while the pre-open drain is running", async () => {
    // The transport both dispatches a frame and queues it for `recv`
    const [gated, peer] = gatedPair();
    const acks = collectAcks(peer);

    const client = SaikuroClient.fromTransport(gated as unknown as Transport);
    const openPromise = client.open();
    // Let `open` register its handler and park in the drain.
    await new Promise((resolve) => setTimeout(resolve, 20));

    const announce = makeAnnounceEnvelope(makeSchemaObject("math", {}));
    await peer.send(announce);
    gated.release(announce as unknown as Record<string, unknown>);

    await openPromise;
    await new Promise((resolve) => setTimeout(resolve, 100));

    expect(acks).toHaveLength(1);
    expect(acks[0]?.["ok"]).toBe(true);
    expect(acks[0]?.["id"]).toEqual(announce.id);

    await client.close();
  });

  it("open() resolves even when the transport recv never yields", async () => {
    const [clientTransport] = InMemoryTransport.pair();
    const client = SaikuroClient.fromTransport(clientTransport);
    // A socket-backed recv waits for the next frame, so open() must not depend
    // on it returning null to finish the drain.
    clientTransport.recv = () => new Promise<null>(() => {});

    await expect(
      Promise.race([
        client.open().then(() => "opened"),
        new Promise((resolve) => setTimeout(() => resolve("hung"), 1_000)),
      ]),
    ).resolves.toBe("opened");

    await client.close();
  });

  it("announce ack is not dispatched to a registered handler", async () => {
    const [clientTransport, providerTransport] = InMemoryTransport.pair();

    const provider = new SaikuroProvider("math");
    // Named after the announce target
    const seen: unknown[] = [];
    provider.register("$saikuro.announce", () => {
      seen.push("dispatched");
      return "wrong";
    });

    const client = SaikuroClient.fromTransport(clientTransport);
    await client.open();
    const servePromise = provider.serveOn(providerTransport);

    await client
      .call("math.missing", [], { timeoutMs: PROMPT_MS })
      .catch((err: unknown) => err);

    expect(seen).toEqual([]);

    await client.close();
    await servePromise;
  });
});
