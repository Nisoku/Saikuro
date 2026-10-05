/**
 * Tests for the server-side WebSocket transport and its listener.
 */

import { describe, it, expect } from "vitest";
import {
  MAX_PENDING_TRANSPORTS,
  WebSocketListener,
  WebSocketServerTransport,
  WebSocketTransport,
} from "../src/transport";

async function waitFor(
  predicate: () => boolean,
  timeoutMs = 2000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (!predicate()) {
    if (Date.now() > deadline) throw new Error("waitFor timed out");
    await new Promise((resolve) => setTimeout(resolve, 5));
  }
}

describe("WebSocketListener / WebSocketServerTransport", () => {
  it("accepts a client and relays frames in both directions", async () => {
    const listener = await WebSocketListener.bind({
      host: "127.0.0.1",
      port: 0,
    });
    try {
      const serverSide: Record<string, unknown>[] = [];
      const clientSide: Record<string, unknown>[] = [];

      const client = new WebSocketTransport(`ws://127.0.0.1:${listener.port}`);
      const [serverTransport] = await Promise.all([
        listener.accept(),
        client.connect(),
      ]);

      serverTransport.onMessage((msg) => serverSide.push(msg));
      client.onMessage((msg) => clientSide.push(msg));

      await client.send({ from: "client" });
      await waitFor(() => serverSide.length === 1);

      await serverTransport.send({ from: "server" });
      await waitFor(() => clientSide.length === 1);

      expect(serverSide[0]).toEqual({ from: "client" });
      expect(clientSide[0]).toEqual({ from: "server" });

      await client.close();
      await serverTransport.close();
    } finally {
      await listener.close();
    }
  });

  it("queues a connection that arrives before accept", async () => {
    const listener = await WebSocketListener.bind({
      host: "127.0.0.1",
      port: 0,
    });
    try {
      const client = new WebSocketTransport(`ws://127.0.0.1:${listener.port}`);
      await client.connect();
      const serverTransport = await listener.accept();
      expect(serverTransport).toBeInstanceOf(WebSocketServerTransport);
      await client.close();
      await serverTransport.close();
    } finally {
      await listener.close();
    }
  });

  it("rejects accept after close", async () => {
    const listener = await WebSocketListener.bind({
      host: "127.0.0.1",
      port: 0,
    });
    await listener.close();
    await expect(listener.accept()).rejects.toThrow("closed");
  });

  it("does not lose a frame sent before accept", async () => {
    const listener = await WebSocketListener.bind({
      host: "127.0.0.1",
      port: 0,
    });
    try {
      const client = new WebSocketTransport(`ws://127.0.0.1:${listener.port}`);
      await client.connect();

      // The peer speaks before the application ever accepts
      await client.send({ early: true });
      await new Promise((resolve) => setTimeout(resolve, 100));

      const serverTransport = await listener.accept();
      const received: Record<string, unknown>[] = [];
      serverTransport.onMessage((msg) => received.push(msg));
      await waitFor(() => received.length === 1);

      expect(received[0]).toEqual({ early: true });

      await client.close();
      await serverTransport.close();
    } finally {
      await listener.close();
    }
  });

  it("rejects an accept that was already waiting when close ran", async () => {
    const listener = await WebSocketListener.bind({
      host: "127.0.0.1",
      port: 0,
    });

    // Nothing will ever connect, so only close() can end this wait.
    const parked = listener.accept();
    await listener.close();

    await expect(parked).rejects.toThrow("closed");
  });

  it("closes a queued connection that nobody accepted", async () => {
    const listener = await WebSocketListener.bind({
      host: "127.0.0.1",
      port: 0,
    });
    const client = new WebSocketTransport(`ws://127.0.0.1:${listener.port}`);
    await client.connect();
    // Registered before close so the close event cannot be missed.
    let clientSawClose = false;
    client.onClose(() => {
      clientSawClose = true;
    });
    await new Promise((resolve) => setTimeout(resolve, 100));

    await listener.close();

    await waitFor(() => clientSawClose);
    await client.close();
  });

  it("reports an error when closing an already-closed listener", async () => {
    const listener = await WebSocketListener.bind({
      host: "127.0.0.1",
      port: 0,
    });
    await listener.close();

    await expect(listener.close()).rejects.toThrow();
  });

  it("settles every accept when two run concurrently", async () => {
    const listener = await WebSocketListener.bind({
      host: "127.0.0.1",
      port: 0,
    });
    try {
      // Both park before either client arrives, so a single-slot waiter would
      // strand the first promise forever.
      const first = listener.accept();
      const second = listener.accept();

      const clientA = new WebSocketTransport(`ws://127.0.0.1:${listener.port}`);
      const clientB = new WebSocketTransport(`ws://127.0.0.1:${listener.port}`);
      await Promise.all([clientA.connect(), clientB.connect()]);

      const transports = await Promise.all([first, second]);
      expect(transports).toHaveLength(2);
      expect(transports[0]).not.toBe(transports[1]);

      await clientA.close();
      await clientB.close();
      await Promise.all(transports.map((t) => t.close()));
    } finally {
      await listener.close();
    }
  });

  it("rejects every parked accept on close", async () => {
    const listener = await WebSocketListener.bind({
      host: "127.0.0.1",
      port: 0,
    });
    const parked = [listener.accept(), listener.accept(), listener.accept()];
    await listener.close();

    for (const promise of parked) {
      await expect(promise).rejects.toThrow("closed");
    }
  });

  it("drops a queued transport whose peer disconnected", async () => {
    const listener = await WebSocketListener.bind({
      host: "127.0.0.1",
      port: 0,
    });
    try {
      const doomed = new WebSocketTransport(`ws://127.0.0.1:${listener.port}`);
      await doomed.connect();
      // Give the server's connection handler time to queue it.
      await new Promise((resolve) => setTimeout(resolve, 100));
      await doomed.close();

      // The dead peer must not be handed to the next accept() caller.
      await new Promise((resolve) => setTimeout(resolve, 100));
      const live = new WebSocketTransport(`ws://127.0.0.1:${listener.port}`);
      const [transport] = await Promise.all([
        listener.accept(),
        live.connect(),
      ]);
      expect(transport.isConnected).toBe(true);

      await live.close();
      await transport.close();
    } finally {
      await listener.close();
    }
  });

  it("refuses connections past the pending limit", async () => {
    const listener = await WebSocketListener.bind({
      host: "127.0.0.1",
      port: 0,
    });
    const clients: WebSocketTransport[] = [];
    let refused = 0;
    try {
      for (let i = 0; i < MAX_PENDING_TRANSPORTS + 3; i++) {
        const client = new WebSocketTransport(
          `ws://127.0.0.1:${listener.port}`,
        );
        // connect() resolves before the server's refusal reaches the client.
        client.onClose(() => {
          refused += 1;
        });
        await client.connect();
        clients.push(client);
      }
      await waitFor(() => refused > 0);
    } finally {
      for (const client of clients) await client.close();
      await listener.close();
    }
  });

  it("settles close while an accepted client is still connected", async () => {
    const listener = await WebSocketListener.bind({
      host: "127.0.0.1",
      port: 0,
    });
    const client = new WebSocketTransport(`ws://127.0.0.1:${listener.port}`);
    const [transport] = await Promise.all([
      listener.accept(),
      client.connect(),
    ]);
    expect(transport.isConnected).toBe(true);

    // ws defers its close callback until every client is gone
    await listener.close();

    await client.close();
    await transport.close();
  });
});
