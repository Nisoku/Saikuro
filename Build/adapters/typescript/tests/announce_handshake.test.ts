/**
 * Tests for the provider/client schema-announce handshake.
 */

import { describe, it, expect } from "vitest";
import { InMemoryTransport } from "../src/transport";
import { SaikuroClient } from "../src/client";
import { SaikuroProvider } from "../src/provider";
import { makeAnnounceEnvelope, makeSchemaObject } from "../src/envelope";
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
