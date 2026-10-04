import { encode, decode } from "@msgpack/msgpack";
import { getLogger } from "../logger";
import { BaseTransport } from "./base";

const log = getLogger("saikuro.transport");

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
  close(cb?: () => void): void;
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
  private readonly _pending: WsSocketLike[] = [];
  private _waiter: ((socket: WsSocketLike) => void) | undefined = undefined;
  private _closed = false;

  private constructor(server: WsServerLike, boundPort: number) {
    this._server = server;
    this._boundPort = boundPort;
    server.on("connection", (socket: WsSocketLike) => {
      if (this._waiter !== undefined) {
        const waiter = this._waiter;
        this._waiter = undefined;
        waiter(socket);
      } else {
        this._pending.push(socket);
      }
    });
    server.on("error", (err: Error) => log.error("ws listener error", { err: String(err) }));
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
      server.on("listening", () => resolve());
      server.on("error", reject);
    });
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
      return Promise.resolve(WebSocketServerTransport.fromSocket(queued));
    }
    return new Promise<WsSocketLike>((resolve) => {
      this._waiter = resolve;
    }).then((socket) => WebSocketServerTransport.fromSocket(socket));
  }

  /** Stop accepting new connections. */
  close(): Promise<void> {
    this._closed = true;
    return new Promise<void>((resolve) => {
      this._server.close(() => resolve());
    });
  }
}
