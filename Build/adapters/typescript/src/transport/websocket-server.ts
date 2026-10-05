import { encode, decode } from "@msgpack/msgpack";
import { getLogger } from "../logger";
import { BaseTransport } from "./base";

const log = getLogger("saikuro.transport");

/** Most unaccepted transports a listener queues before refusing new peers. */
export const MAX_PENDING_TRANSPORTS = 64;

//  WebSocket server transport (Node.js)

interface WsSocketLike {
  send(data: Uint8Array): void;
  close(code?: number, reason?: string): void;
  on(event: "message", cb: (data: Buffer, isBinary: boolean) => void): void;
  on(event: "close", cb: (code: number, reason: Buffer) => void): void;
  on(event: "error", cb: (err: Error) => void): void;
}

/** Structural view of `ws`'s `WebSocketServer`. */
interface WsServerLike {
  on(event: "connection", cb: (socket: WsSocketLike) => void): void;
  on(event: "listening", cb: () => void): void;
  on(event: "error", cb: (err: Error) => void): void;
  off(event: "error", cb: (err: Error) => void): void;
  close(cb?: (err?: Error) => void): void;
  address(): { address: string; port: number } | string | null;
}

/**
 * Server-side counterpart of {@link WebSocketTransport}: wraps a socket that
 * was accepted by a {@link WebSocketListener}.
 */
export class WebSocketServerTransport extends BaseTransport {
  private readonly _socket: WsSocketLike;
  private _connected = true;

  private constructor(socket: WsSocketLike) {
    super();
    this._socket = socket;
    socket.on("message", (data: Buffer, isBinary: boolean) => {
      if (!isBinary) {
        // Text frames are not part of the binary MessagePack protocol.
        log.warn("ws server ignoring text frame");
        return;
      }
      try {
        this._dispatch(decode(data) as Record<string, unknown>);
      } catch (err) {
        log.error("ws server frame decode error", { err: String(err) });
      }
    });
    socket.on("close", () => {
      this._connected = false;
      this._closeHandler?.();
    });
    socket.on("error", (err: Error) => {
      this._connected = false;
      this._closeHandler?.(err);
    });
  }

  /** Adopt an already-upgraded server socket. */
  static fromSocket(socket: WsSocketLike): WebSocketServerTransport {
    return new WebSocketServerTransport(socket);
  }

  /** Already connected on construction. */
  async connect(): Promise<void> {}

  async close(): Promise<void> {
    if (!this._connected) return;
    this._connected = false;
    this._socket.close(1000, "normal closure");
  }

  async send(obj: object): Promise<void> {
    if (!this._connected) throw new Error("WebSocket is not connected");
    this._socket.send(encode(obj) as Uint8Array);
  }

  async recv(): Promise<Record<string, unknown> | null> {
    return null;
  }

  /** Whether the underlying socket is still open. */
  get isConnected(): boolean {
    return this._connected;
  }
}

/** Options for {@link WebSocketListener.bind}. */
export interface WebSocketListenerOptions {
  /** Interface to bind. Defaults to all interfaces. */
  host?: string;
  /** Port to bind. Defaults to `0` (an ephemeral port). */
  port?: number;
}

/**
 * Accepts inbound WebSocket connections and yields a
 * {@link WebSocketServerTransport} for each.
 */
export class WebSocketListener {
  private readonly _server: WsServerLike;
  private readonly _boundPort: number;
  private readonly _pending: WebSocketServerTransport[] = [];
  /** Transports already handed to an accept() caller; shutdown must close them. */
  private readonly _accepted = new Set<WebSocketServerTransport>();
  private readonly _waiters: Array<{
    resolve: (transport: WebSocketServerTransport) => void;
    reject: (err: Error) => void;
  }> = [];
  private _closed = false;

  private constructor(server: WsServerLike, boundPort: number) {
    this._server = server;
    this._boundPort = boundPort;
    server.on("connection", (socket: WsSocketLike) => {
      if (this._closed) {
        // A connection that races close() must not be left hanging on a peer
        // that will never be served.
        socket.close(1001, "server closed");
        return;
      }
      if (this._pending.length >= MAX_PENDING_TRANSPORTS) {
        socket.close(1013, "too many pending connections");
        return;
      }
      // Wrap the socket here rather than in accept()
      const transport = WebSocketServerTransport.fromSocket(socket);
      // A peer that disappears before it is accepted must not sit in the queue
      // forever, and must not keep shutdown waiting on it.
      socket.on("close", () => this._forget(transport));
      socket.on("error", () => this._forget(transport));
      const waiter = this._waiters.shift();
      if (waiter !== undefined) {
        this._accepted.add(transport);
        waiter.resolve(transport);
      } else {
        this._pending.push(transport);
      }
    });
  }

  /**
   * Bind a WebSocket listener. Rejects if `ws` is not installed or the socket
   * cannot be bound.
   */
  static async bind(
    options: WebSocketListenerOptions = {},
  ): Promise<WebSocketListener> {
    let WebSocketServer: new (opts: {
      host?: string;
      port: number;
    }) => WsServerLike;
    try {
      const mod = (await import("ws")) as unknown as {
        WebSocketServer: typeof WebSocketServer;
      };
      WebSocketServer = mod.WebSocketServer;
    } catch (err) {
      throw new Error("WebSocketListener requires the 'ws' package", {
        cause: err,
      });
    }
    const server = new WebSocketServer({
      ...(options.host !== undefined ? { host: options.host } : {}),
      port: options.port ?? 0,
    });
    await new Promise<void>((resolve, reject) => {
      const onBindError = (err: Error): void => {
        reject(err);
      };
      server.on("error", onBindError);
      server.on("listening", () => {
        // The bind-scoped rejection must not outlive the bind
        server.off("error", onBindError);
        resolve();
      });
    });
    // Long-lived replacement for the bind-scoped listener.
    server.on("error", (err: Error) =>
      log.error("ws listener error", { err: String(err) }),
    );
    const address = server.address();
    const boundPort =
      typeof address === "object" && address !== null ? address.port : 0;
    return new WebSocketListener(server, boundPort);
  }

  /** The port the listener is bound to. */
  get port(): number {
    return this._boundPort;
  }

  /** Wait for the next inbound connection. */
  accept(): Promise<WebSocketServerTransport> {
    if (this._closed) {
      return Promise.reject(new Error("WebSocketListener is closed"));
    }
    const queued = this._pending.shift();
    if (queued !== undefined) {
      this._accepted.add(queued);
      return Promise.resolve(queued);
    }
    return new Promise<WebSocketServerTransport>((resolve, reject) => {
      this._waiters.push({ resolve, reject });
    });
  }

  /** Drop a transport whose peer went away. */
  private _forget(transport: WebSocketServerTransport): void {
    const index = this._pending.indexOf(transport);
    if (index !== -1) {
      this._pending.splice(index, 1);
    }
    this._accepted.delete(transport);
  }

  /** Close a transport whose close failure must not abort shutdown. */
  private _closeQuietly(transport: WebSocketServerTransport): void {
    transport.close().catch((err: unknown) => {
      log.debug("ws server transport close failed", { err: String(err) });
    });
  }

  /**
   * Stop accepting new connections.
   */
  close(): Promise<void> {
    this._closed = true;

    // Every parked accept() has to be woken
    for (const waiter of this._waiters.splice(0)) {
      waiter.reject(new Error("WebSocketListener is closed"));
    }

    // Transports nobody accepted belong to peers that would otherwise hold an
    // open connection nothing will ever serve.
    for (const transport of this._pending.splice(0)) {
      this._closeQuietly(transport);
    }

    // ws only settles close() once every client is gone, so the ones already
    // handed out have to be closed first or close() never resolves.
    for (const transport of this._accepted) {
      this._closeQuietly(transport);
    }

    return new Promise<void>((resolve, reject) => {
      this._server.close((err?: Error) => {
        if (err !== undefined) {
          reject(err);
          return;
        }
        resolve();
      });
    });
  }
}
