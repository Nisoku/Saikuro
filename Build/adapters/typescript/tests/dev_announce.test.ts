import { describe, it, expect } from "vitest";
import { SaikuroProvider } from "../src/provider";
import { InMemoryTransport } from "../src/transport";
import { resolve } from "path";

describe("SaikuroProvider dev-mode announce", () => {
  it("extracts schema from source files and announces it when serving in dev mode", async () => {
    const [sender, receiver] = InMemoryTransport.pair();
    const provider = new SaikuroProvider("testns");

    // Start serving on the in-memory transport in dev mode with the fixture
    const fixture = resolve(__dirname, "./fixtures/service.ts");

    // Arrange receiver to capture the announce and reply with an ack so the
    // provider's _announce can complete.
    let seen: any = null;
    receiver.onMessage(async (m) => {
      seen = m;
      // Reply with a positive ack back to the provider (sent to the peer)
      await receiver.send({ id: m.id, ok: true } as any);
    });

    // Kick off serve (it will call announce and then run serve loop waiting for close).
    const servePromise = provider.serveOn(sender, {
      dev: true,
      sourceFiles: [fixture],
    });

    // Give the async announce a moment to run
    await new Promise((r) => setTimeout(r, 20));

    expect(seen).toBeTruthy();
    const ns = (seen.args?.[0]?.namespaces ?? {})["testns"];
    expect(ns).toBeTruthy();
    expect(ns.functions).toHaveProperty("add");

    // Cleanup
    await sender.close();
    await servePromise.catch(() => {});
  });
});
