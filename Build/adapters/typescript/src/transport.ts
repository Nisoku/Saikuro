/**
 * Transport abstraction for the TypeScript Saikuro adapter.
 *
 * Implementations:
 *   - NodeUnixTransport  - Unix domain socket (Node.js, native only)
 *   - NodeTcpTransport   - TCP (Node.js, native only)
 *   - WebSocketTransport - WebSocket (Node.js + browser + WASM)
 *   - InMemoryTransport  - In-process (testing)
 *
 * All implement the `Transport` interface.
 */

import { encode, decode } from "@msgpack/msgpack";
import { getLogger } from "./logger";

const log = getLogger("saikuro.transport");

//  Interface

export interface Transport {
  /** Connect (or initialise) the transport. */
  connect(): Promise<void>;

  /** Close the transport gracefully. */
  close(): Promise<void>;

  /** Serialise `obj` to MessagePack and transmit it. */
  send(obj: Record<string, unknown>): Promise<void>;

  /**
   * Receive and deserialise the next message.
   * Returns `null` when the connection has been closed by the peer.
   */
  recv(): Promise<Record<string, unknown> | null>;

  /** Register a handler invoked for every received message.
   * Multiple handlers may be registered; all are called in registration order.
   */
  onMessage(handler: (msg: Record<string, unknown>) => void): void;

  /**
   * Deregister a previously-registered message handler.
   * A no-op if the handler was never registered.
   */
  offMessage(handler: (msg: Record<string, unknown>) => void): void;

  /** Register an error / close handler. */
  onClose(handler: (err?: Error) => void): void;
}

//  Frame codec (length-prefix, big-endian uint32)

const MAX_FRAME_SIZE = 16 * 1024 * 1024; // 16 MiB

function buildFrame(payload: Uint8Array): Buffer {
  const header = Buffer.allocUnsafe(4);
  header.writeUInt32BE(payload.length, 0);
  return Buffer.concat([header, payload]);
}

//  In-Memory Transport (testing)

export class InMemoryTransport implements Transport {
  private readonly _inbox: Array<Record<string, unknown>> = [];
  private readonly _messageHandlers = new Set<
    (msg: Record<string, unknown>) => void
  >();
  private _closeHandler?: (err?: Error) => void;
  private _peer?: InMemoryTransport;
  private _closed = false;

  /** Create a connected pair of in-memory transports. */
  static pair(): [InMemoryTransport, InMemoryTransport] {
    const a = new InMemoryTransport();
    const b = new InMemoryTransport();
    a._peer = b;
    b._peer = a;
    return [a, b];
  }

  async connect(): Promise<void> {
    /* no-op */
  }

  async close(): Promise<void> {
    this._closed = true;
    this._closeHandler?.();
    this._peer?._closeHandler?.();
  }

  async send(obj: Record<string, unknown>): Promise<void> {
    if (this._closed) throw new Error("transport is closed");
    // Snapshot the handler set before iterating so that handlers which call
    // offMessage/onMessage during delivery cannot corrupt the iteration or
    // cause re-entrant infinite loops.
    const handlers = Array.from(this._peer?._messageHandlers ?? []);
    for (const h of handlers) h(obj);
  }

  async recv(): Promise<Record<string, unknown> | null> {
    return this._inbox.shift() ?? null;
  }

  onMessage(handler: (msg: Record<string, unknown>) => void): void {
    this._messageHandlers.add(handler);
  }

  offMessage(handler: (msg: Record<string, unknown>) => void): void {
    this._messageHandlers.delete(handler);
  }

  onClose(handler: (err?: Error) => void): void {
    this._closeHandler = handler;
  }
}

//  WebSocket Transport (browser + Node.js + WASM)

/**
 * A transport backed by the browser/Node.js WebSocket API.
 *
 * On Node.js >= 22 the built-in `WebSocket` global is available.
 * For older versions pass a `ws`-compatible constructor via `wsConstructor`.
 */
export class WebSocketTransport implements Transport {
  private _ws?: WebSocket;
  private readonly _messageHandlers = new Set<
    (msg: Record<string, unknown>) => void
  >();
  private _closeHandler?: (err?: Error) => void;
  private readonly _url: string;
  private readonly _WS: typeof WebSocket;

  constructor(url: string, options?: { wsConstructor?: typeof WebSocket }) {
    this._url = url;
    if (options?.wsConstructor !== undefined) {
      this._WS = options.wsConstructor;
    } else if (typeof globalThis.WebSocket !== "undefined") {
      this._WS = globalThis.WebSocket;
    } else {
      throw new Error(
        "WebSocket is not available in this environment. " +
          "Pass a wsConstructor (e.g. from the 'ws' package).",
      );
    }
  }

  connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      const ws = new this._WS(this._url);
      ws.binaryType = "arraybuffer";
      ws.onopen = () => {
        this._ws = ws;
        resolve();
      };
      ws.onerror = (ev) => {
        reject(new Error(`WebSocket connect failed: ${String(ev)}`));
      };
      ws.onmessage = (ev) => {
        try {
          const bytes = new Uint8Array(ev.data as ArrayBuffer);
          const msg = decode(bytes) as Record<string, unknown>;
          // Snapshot handlers so any onMessage/offMessage calls inside a
          // handler do not corrupt the current dispatch pass.
          const handlers = Array.from(this._messageHandlers);
          for (const h of handlers) h(msg);
        } catch (err) {
          // Malformed frame - log and continue; don't crash the receive loop.
          log.error("ws frame decode error", { err: String(err) });
        }
      };
      ws.onclose = (ev) => {
        const err = ev.wasClean
          ? undefined
          : new Error(`WebSocket closed unexpectedly: code=${ev.code}`);
        this._closeHandler?.(err);
      };
    });
  }

  async close(): Promise<void> {
    this._ws?.close(1000, "normal closure");
  }

  async send(obj: Record<string, unknown>): Promise<void> {
    if (this._ws === undefined || this._ws.readyState !== WebSocket.OPEN) {
      throw new Error("WebSocket is not connected");
    }
    const payload = encode(obj);
    this._ws.send(payload);
  }

  async recv(): Promise<Record<string, unknown> | null> {
    // Receive is event-driven via onMessage; this method is a no-op stub.
    return null;
  }

  onMessage(handler: (msg: Record<string, unknown>) => void): void {
    this._messageHandlers.add(handler);
  }

  offMessage(handler: (msg: Record<string, unknown>) => void): void {
    this._messageHandlers.delete(handler);
  }

  onClose(handler: (err?: Error) => void): void {
    this._closeHandler = handler;
  }
}

//  Node.js TCP / Unix transport (conditional import)

/**
 * Node.js stream-based transport (TCP or Unix socket).
 *
 * Uses a dynamic `import("net")` so the module can be bundled for
 * browser/WASM targets without errors - those targets simply never call this
 * class.
 *
 * Framing: 4-byte big-endian length prefix (matches the Rust LengthPrefixedCodec).
 */
export class NodeStreamTransport implements Transport {
  private _socket?: import("net").Socket;
  private readonly _messageHandlers = new Set<
    (msg: Record<string, unknown>) => void
  >();
  private _closeHandler?: (err?: Error) => void;
  private _buffer: Buffer = Buffer.alloc(0);
  private readonly _connectionOptions:
    | { type: "tcp"; host: string; port: number }
    | { type: "unix"; path: string };

  static tcp(host: string, port: number): NodeStreamTransport {
    return new NodeStreamTransport({ type: "tcp", host, port });
  }

  static unix(path: string): NodeStreamTransport {
    return new NodeStreamTransport({ type: "unix", path });
  }

  private constructor(
    opts:
      | { type: "tcp"; host: string; port: number }
      | { type: "unix"; path: string },
  ) {
    this._connectionOptions = opts;
  }

  async connect(): Promise<void> {
    const net = await import("net");
    return new Promise((resolve, reject) => {
      const opts = this._connectionOptions;
      const connectArgs =
        opts.type === "tcp"
          ? { host: opts.host, port: opts.port }
          : { path: opts.path };

      const socket = net.createConnection(
        connectArgs as unknown as Parameters<typeof net.createConnection>[0],
        () => {
          this._socket = socket;
          resolve();
        },
      );

      socket.on("error", reject);
      socket.on("data", (chunk: Buffer) => this._onData(chunk));
      socket.on("close", (hadError: boolean) => {
        const err = hadError
          ? new Error("socket closed with error")
          : undefined;
        this._closeHandler?.(err);
      });
    });
  }

  private _onData(chunk: Buffer): void {
    this._buffer = Buffer.concat([this._buffer, chunk]);

    while (this._buffer.length >= 4) {
      const frameLen = this._buffer.readUInt32BE(0);
      if (frameLen > MAX_FRAME_SIZE) {
        this._closeHandler?.(new Error(`frame too large: ${frameLen} bytes`));
        return;
      }
      if (this._buffer.length < 4 + frameLen) break;

      const payload = this._buffer.subarray(4, 4 + frameLen);
      this._buffer = this._buffer.subarray(4 + frameLen);

      try {
        const msg = decode(payload) as Record<string, unknown>;
        // Snapshot handlers so any onMessage/offMessage calls inside a
        // handler do not corrupt the current dispatch pass.
        const handlers = Array.from(this._messageHandlers);
        for (const h of handlers) h(msg);
      } catch (err) {
        log.error("frame decode error", { err: String(err) });
      }
    }
  }

  async close(): Promise<void> {
    this._socket?.end();
  }

  async send(obj: Record<string, unknown>): Promise<void> {
    if (this._socket === undefined) throw new Error("not connected");
    const payload = encode(obj) as Uint8Array;
    const frame = buildFrame(payload);
    await new Promise<void>((resolve, reject) => {
      this._socket!.write(frame, (err) => (err ? reject(err) : resolve()));
    });
  }

  async recv(): Promise<Record<string, unknown> | null> {
    // Event-driven via onMessage.
    return null;
  }

  onMessage(handler: (msg: Record<string, unknown>) => void): void {
    this._messageHandlers.add(handler);
  }

  offMessage(handler: (msg: Record<string, unknown>) => void): void {
    this._messageHandlers.delete(handler);
  }

  onClose(handler: (err?: Error) => void): void {
    this._closeHandler = handler;
  }
}

//  Factory

/**
 * Construct the best transport for a given address string.
 *
 * Formats:
 *   - `unix:///path/to/socket`
 *   - `tcp://host:port`
 *   - `ws://host:port/path`  or  `wss://host:port/path`
 */
export function makeTransport(address: string): Transport {
  if (address.startsWith("unix://")) {
    return NodeStreamTransport.unix(address.slice("unix://".length));
  }
  if (address.startsWith("tcp://")) {
    const rest = address.slice("tcp://".length);
    const lastColon = rest.lastIndexOf(":");
    const host = rest.slice(0, lastColon);
    const port = parseInt(rest.slice(lastColon + 1), 10);
    return NodeStreamTransport.tcp(host, port);
  }
  if (address.startsWith("ws://") || address.startsWith("wss://")) {
    return new WebSocketTransport(address);
  }
  throw new Error(
    `unsupported transport address: "${address}"\n` +
      "Supported schemes: unix://, tcp://, ws://, wss://",
  );
}
