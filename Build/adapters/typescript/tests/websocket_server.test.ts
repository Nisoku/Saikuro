/**
 * Tests for the server-side WebSocket transport and its listener.
 */

import { describe, it, expect } from "vitest";
import {
  WebSocketListener,
  WebSocketServerTransport,
  WebSocketTransport,
} from "../src/transport";

async function waitFor(predicate: () => boolean, timeoutMs = 2000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (!predicate()) {
    if (Date.now() > deadline) throw new Error("waitFor timed out");
    await new Promise((resolve) => setTimeout(resolve, 5));
  }
}

describe("WebSocketListener / WebSocketServerTransport", () => {
  it("accepts a client and relays frames in both directions", async () => {
    const listener = await WebSocketListener.bind({ host: "127.0.0.1", port: 0 });
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
    const listener = await WebSocketListener.bind({ host: "127.0.0.1", port: 0 });
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
    const listener = await WebSocketListener.bind({ host: "127.0.0.1", port: 0 });
    await listener.close();
    await expect(listener.accept()).rejects.toThrow("closed");
  });
});
