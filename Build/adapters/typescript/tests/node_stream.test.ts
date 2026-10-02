/**
 * Tests for the Node.js stream transport
 */

import { describe, it, expect, afterEach, vi } from "vitest";
import { createServer, type Server, type Socket } from "node:net";
import { encode } from "@msgpack/msgpack";
import { NodeStreamTransport } from "../src/transport";
import { buildFrame } from "../src/transport/framing";

/** Address used for loopback binds; port 0 lets the OS pick a free one. */
const LOOPBACK = "127.0.0.1";

const openServers: Server[] = [];
const openSockets: Socket[] = [];

/**
 * Close everything opened during a test.
 */
afterEach(async () => {
  for (const socket of openSockets.splice(0)) socket.destroy();
  for (const server of openServers.splice(0)) {
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
});

/**
 * Start a loopback TCP server, dial it once, and resolve with the transport
 * wrapping the accepted socket plus the transport on the dialing side.
 */
async function connectedPair(): Promise<{
  serverSide: NodeStreamTransport;
  clientSide: NodeStreamTransport;
}> {
  const server = createServer();
  openServers.push(server);
  await new Promise<void>((resolve) => {
    server.listen(0, LOOPBACK, resolve);
  });

  const address = server.address();
  if (address === null || typeof address === "string") {
    throw new Error("expected an IP socket address");
  }

  const accepted = new Promise<Socket>((resolve) => {
    server.once("connection", (socket) => resolve(socket));
  });

  const clientSide = NodeStreamTransport.tcp(LOOPBACK, address.port);
  await clientSide.connect();

  const socket = await accepted;
  openSockets.push(socket);
  return { serverSide: NodeStreamTransport.fromSocket(socket), clientSide };
}

describe("NodeStreamTransport accept path", () => {
  it("exchanges framed objects in both directions over a real socket", async () => {
    const { serverSide, clientSide } = await connectedPair();

    const atServer: Array<Record<string, unknown>> = [];
    const atClient: Array<Record<string, unknown>> = [];
    serverSide.onMessage((msg) => atServer.push(msg));
    clientSide.onMessage((msg) => atClient.push(msg));

    await clientSide.send({ kind: "request", n: 1 });
    await serverSide.send({ kind: "response", n: 2 });

    await vi.waitFor(() => {
      expect(atServer).toEqual([{ kind: "request", n: 1 }]);
      expect(atClient).toEqual([{ kind: "response", n: 2 }]);
    });

    await clientSide.close();
    await serverSide.close();
  });

  it("preserves a payload larger than one socket chunk", async () => {
    const { serverSide, clientSide } = await connectedPair();

    const received: Array<Record<string, unknown>> = [];
    serverSide.onMessage((msg) => received.push(msg));

    // Larger than a typical socket write, so it only decodes correctly if the
    // length prefix is reassembled across chunks.
    const big = "x".repeat(200_000);
    await clientSide.send({ big });

    await vi.waitFor(() => {
      expect(received).toHaveLength(1);
      expect(received[0]?.["big"]).toBe(big);
    });

    await clientSide.close();
    await serverSide.close();
  });

  it("treats connect() on a transport built from an accepted socket as a no-op", async () => {
    const { serverSide, clientSide } = await connectedPair();

    await expect(serverSide.connect()).resolves.toBeUndefined();

    const received: Record<string, unknown>[] = [];
    serverSide.onMessage((msg) => received.push(msg));
    await clientSide.send({ ping: 1 });
    await vi.waitFor(() => expect(received).toHaveLength(1));
    expect(received[0]).toEqual({ ping: 1 });

    await clientSide.close();
    await serverSide.close();
  });

  it("stops reporting live once the peer closes, and refuses to redial an adopted socket", async () => {
    const { serverSide, clientSide } = await connectedPair();

    expect(serverSide.isConnected).toBe(true);
    await clientSide.close();

    await vi.waitFor(() => expect(serverSide.isConnected).toBe(false));

    // Adoption carries no dial target, so this must be an explicit refusal
    // rather than silently reporting success while still dead.
    await expect(serverSide.connect()).rejects.toThrow("no dial target");
  });

  it("redials a tcp transport after the peer closes", async () => {
    const sessions: Socket[] = [];
    const server = createServer((socket) => sessions.push(socket));
    openServers.push(server);
    await new Promise<void>((resolve) => {
      server.listen(0, LOOPBACK, resolve);
    });
    const address = server.address();
    if (address === null || typeof address === "string") {
      throw new Error("expected an IP socket address");
    }

    const clientSide = NodeStreamTransport.tcp(LOOPBACK, address.port);
    await clientSide.connect();

    await vi.waitFor(() => expect(sessions).toHaveLength(1));
    sessions[0].destroy();
    await vi.waitFor(() => expect(clientSide.isConnected).toBe(false));

    await clientSide.connect();
    expect(clientSide.isConnected).toBe(true);
    await vi.waitFor(() => expect(sessions).toHaveLength(2));

    const received: Record<string, unknown>[] = [];
    clientSide.onMessage((msg) => received.push(msg));
    const redialed = sessions[sessions.length - 1]!;
    redialed.write(buildFrame(encode({ ping: 9 }) as Uint8Array));
    await vi.waitFor(() => expect(received).toHaveLength(1));
    expect(received[0]).toEqual({ ping: 9 });

    await clientSide.close();
    for (const socket of sessions) socket.destroy();
  });

  it("surfaces a clean peer close without an error", async () => {
    const { serverSide, clientSide } = await connectedPair();

    let closedWithError = true;
    serverSide.onClose((err) => {
      closedWithError = err !== undefined;
    });

    await clientSide.close();

    await vi.waitFor(() => {
      expect(closedWithError).toBe(false);
    });

    await serverSide.close();
  });
});
