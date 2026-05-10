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

  /** Register a handler invoked for every received message. */
  onMessage(handler: (msg: Record<string, unknown>) => void): void;

  /** Deregister a previously-registered message handler. */
  offMessage(handler: (msg: Record<string, unknown>) => void): void;

  /** Register an error / close handler. */
  onClose(handler: (err?: Error) => void): void;
}

/**
 * Shared handler management for all transports.
 *
 * Every transport implementation needs the same `_messageHandlers` set,
 * `_closeHandler`, and the three registration methods.  This base class
 * provides them so each transport only writes the unique parts.
 */
abstract class BaseTransport implements Transport {
  readonly _messageHandlers = new Set<
    (msg: Record<string, unknown>) => void
  >();
  _closeHandler?: (err?: Error) => void;

  abstract connect(): Promise<void>;
  abstract close(): Promise<void>;
  abstract send(obj: Record<string, unknown>): Promise<void>;
  abstract recv(): Promise<Record<string, unknown> | null>;

  onMessage(handler: (msg: Record<string, unknown>) => void): void {
    this._messageHandlers.add(handler);
  }

  offMessage(handler: (msg: Record<string, unknown>) => void): void {
    this._messageHandlers.delete(handler);
  }

  onClose(handler: (err?: Error) => void): void {
    this._closeHandler = handler;
  }

  /** Snapshot the current handler set and dispatch to each in registration order. */
  _dispatch(msg: Record<string, unknown>): void {
    for (const h of Array.from(this._messageHandlers)) h(msg);
  }
}

//  Frame codec (length-prefix, big-endian uint32)

const MAX_FRAME_SIZE = 16 * 1024 * 1024; // 16 MiB

function buildFrame(payload: Uint8Array): Buffer {
  const header = Buffer.allocUnsafe(4);
  header.writeUInt32BE(payload.length, 0);
  return Buffer.concat([header, payload]);
}

//  In-Memory Transport (testing)

export class InMemoryTransport extends BaseTransport {
  private readonly _inbox: Array<Record<string, unknown>> = [];
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
    for (const h of Array.from(this._peer?._messageHandlers ?? [])) h(obj);
  }

  async recv(): Promise<Record<string, unknown> | null> {
    return this._inbox.shift() ?? null;
  }
}

//  WebSocket Transport (browser + Node.js + WASM)

/**
 * A transport backed by the browser/Node.js WebSocket API.
 *
 * On Node.js >= 22 the built-in `WebSocket` global is available.
 * For older versions pass a `ws`-compatible constructor via `wsConstructor`.
 */
export class WebSocketTransport extends BaseTransport {
  private _ws?: WebSocket;
  private readonly _url: string;
  private readonly _WS: typeof WebSocket;

  constructor(url: string, options?: { wsConstructor?: typeof WebSocket }) {
    super();
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
          this._dispatch(msg);
        } catch (err) {
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
    return null;
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
export class NodeStreamTransport extends BaseTransport {
  private _socket?: import("net").Socket;
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
    super();
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
        this._dispatch(msg);
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
    return null;
  }
}

//  BroadcastChannel Transport (browser/WASM in-page communication)

/**
 * Creates a transport from an already-connected BroadcastChannel.
 */
function makeTransportFromChannel(
  channel: BroadcastChannel,
): BroadcastChannelTransport {
  const t = new BroadcastChannelTransport(channel.name);
  (t as unknown as { _channel: BroadcastChannel })._channel = channel;
  (t as unknown as { _connected: boolean })._connected = true;
  (t as unknown as { _setupMessageHandler: () => void })._setupMessageHandler();
  return t;
}

/**
 * Generate a short random ID for connection negotiation.
 * Format: 8 hex chars (4 from timestamp, 4 random).
 * Not guaranteed to be universally unique, but should be good enough for
 * avoiding collisions in the connection handshake.
 */
function generateShortId(): string {
  const now = Date.now() & 0xffff;
  const rand = Math.floor(Math.random() * 0x10000);
  return `${now.toString(16).padStart(4, "0")}${rand.toString(16).padStart(4, "0")}`;
}

/**
 * A transport backed by the browser's BroadcastChannel API.
 *
 * Uses the same connection negotiation protocol as the Rust WasmHostTransport:
 * 1. Connector generates a random connection ID
 * 2. Connector sends `{ type: "connect", id: conn_id }` on base channel
 * 3. Listener receives, opens private channel `{base}:{conn_id}`
 * 4. Listener sends `{ type: "accept", id: conn_id }` on private channel
 * 5. Both sides communicate on private channel from then on
 *
 * This transport is for communication between different JavaScript/WASM contexts
 * in the same browser origin (e.g., main page ↔ Worker, or JS ↔ Rust WASM).
 */
export class BroadcastChannelTransport extends BaseTransport {
  private _channel: BroadcastChannel | undefined = undefined;
  private readonly _baseChannel: string;
  private _connected = false;
  private _closed = false;

  /**
   * Create a BroadcastChannelTransport for the "wasm-host://" scheme.
   *
   * Call `connect()` to perform the rendezvous handshake.
   */
  constructor(baseChannel: string) {
    super();
    this._baseChannel = baseChannel;
  }

  private _setupMessageHandler(): void {
    const ch = this._channel;
    if (!ch) return;
    ch.onmessage = (ev: MessageEvent) => {
      try {
        let msg: Record<string, unknown>;
        if (ev.data instanceof ArrayBuffer) {
          const bytes = new Uint8Array(ev.data);
          msg = decode(bytes) as Record<string, unknown>;
        } else if (ev.data instanceof Uint8Array) {
          msg = decode(ev.data) as Record<string, unknown>;
        } else {
          return;
        }
        this._dispatch(msg);
      } catch (err) {
        log.error("broadcast channel frame decode error", {
          err: String(err),
        });
      }
    };
  }

  /**
   * Connect as a client using the rendezvous protocol.
   * This generates a connection ID and waits for a listener to accept.
   */
  connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      if (this._connected) {
        resolve();
        return;
      }

      const connId = generateShortId();
      const privateName = `${this._baseChannel}:${connId}`;
      const privateChannel = new BroadcastChannel(privateName);

      const timeoutId = setTimeout(() => {
        privateChannel.close();
        reject(new Error("BroadcastChannelTransport: connect timeout"));
      }, 10000);

      const acceptHandler = (_ev: MessageEvent) => {
        clearTimeout(timeoutId);
        privateChannel.removeEventListener("message", acceptHandler);
        this._channel = privateChannel;
        this._connected = true;
        this._setupMessageHandler();
        resolve();
      };
      privateChannel.addEventListener("message", acceptHandler, {
        once: true,
      });

      const baseChannel = new BroadcastChannel(this._baseChannel);
      baseChannel.postMessage({ type: "connect", id: connId });
      baseChannel.close();
    });
  }

  async close(): Promise<void> {
    if (this._closed) return;
    this._closed = true;
    this._connected = false;
    this._channel?.close();
    this._channel = undefined;
    this._closeHandler?.();
  }

  async send(obj: Record<string, unknown>): Promise<void> {
    const ch = this._channel;
    if (!this._connected || !ch) {
      throw new Error("BroadcastChannelTransport: not connected");
    }
    const payload = encode(obj);
    const buffer = payload.buffer.slice(
      payload.byteOffset,
      payload.byteOffset + payload.byteLength,
    );
    ch.postMessage(buffer);
  }

  async recv(): Promise<Record<string, unknown> | null> {
    return null;
  }
}

/**
 * Connector for initiating BroadcastChannel connections.
 * Matches the Rust WasmHostConnector behavior.
 */
export class BroadcastChannelConnector {
  private readonly _channelName: string;

  constructor(channelName: string) {
    this._channelName = channelName;
  }

  connect(): Promise<BroadcastChannelTransport> {
    const t = new BroadcastChannelTransport(this._channelName);
    return t.connect().then(() => t);
  }
}

/**
 * Listener for accepting incoming BroadcastChannel connections.
 * Matches the Rust WasmHostListener behavior.
 */
export class BroadcastChannelListener {
  private readonly _baseChannel: BroadcastChannel;
  private readonly _connectQueue: Array<(t: BroadcastChannelTransport) => void> =
    [];
  private _closed = false;

  constructor(channelName: string) {
    this._baseChannel = new BroadcastChannel(channelName);
    this._baseChannel.onmessage = (ev: MessageEvent) => {
      const data = ev.data as { type?: string; id?: string };
      if (data.type === "connect" && data.id) {
        const privateName = `${channelName}:${data.id}`;
        const privateChannel = new BroadcastChannel(privateName);

        privateChannel.postMessage({ type: "accept", id: data.id });

        const transport = makeTransportFromChannel(privateChannel);

        const queued = this._connectQueue.shift();
        if (queued) {
          queued(transport);
        }
      }
    };
  }

  /**
   * Wait for the next incoming connection.
   */
  accept(): Promise<BroadcastChannelTransport> {
    return new Promise((resolve) => {
      if (this._closed) {
        throw new Error("BroadcastChannelListener is closed");
      }
      this._connectQueue.push(resolve);
    });
  }

  async close(): Promise<void> {
    if (this._closed) return;
    this._closed = true;
    this._baseChannel.close();
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
 *   - `wasm-host://channel-name` (BroadcastChannel for same-origin browser contexts)
 *   - `wasm-host` (uses default channel "saikuro")
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
  if (address === "wasm-host" || address.startsWith("wasm-host://")) {
    let channelName = "saikuro";
    if (address.startsWith("wasm-host://")) {
      const rest = address.slice("wasm-host://".length);
      if (rest.length > 0) {
        channelName = rest;
      }
    }
    return new BroadcastChannelTransport(channelName);
  }
  throw new Error(
    `unsupported transport address: "${address}"\n` +
      "Supported schemes: unix://, tcp://, ws://, wss://, wasm-host://",
  );
}
