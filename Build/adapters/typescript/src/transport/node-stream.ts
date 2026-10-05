import { encode, decode } from "@msgpack/msgpack";
import { getLogger } from "../logger";
import { BaseTransport } from "./base";
import { MAX_FRAME_SIZE, buildFrame } from "./framing";

const log = getLogger("saikuro.transport");

//  Node.js TCP / Unix transport (conditional import)

/**
 * Node.js stream-based transport (TCP or Unix socket).
 *
 * Uses a dynamic `import("net")` so the module can be bundled for
 * browser/WASM targets without errors, those targets simply never call this
 * class.
 *
 * Framing: 4-byte big-endian length prefix (matches the Rust LengthPrefixedCodec).
 */
export class NodeStreamTransport extends BaseTransport {
  private _socket?: import("net").Socket;
  private _buffer: Buffer = Buffer.alloc(0);
  private _connected = false;
  private readonly _connectionOptions:
    | { type: "tcp"; host: string; port: number }
    | { type: "unix"; path: string }
    | { type: "socket" };

  static tcp(host: string, port: number): NodeStreamTransport {
    return new NodeStreamTransport({ type: "tcp", host, port });
  }

  static unix(path: string): NodeStreamTransport {
    return new NodeStreamTransport({ type: "unix", path });
  }

  private constructor(
    opts:
      | { type: "tcp"; host: string; port: number }
      | { type: "unix"; path: string }
      | { type: "socket" },
  ) {
    super();
    this._connectionOptions = opts;
  }

  /**
   * Wrap an already-connected socket, e.g. one accepted from a
   * `net.createServer` listener.
   */
  static fromSocket(socket: import("net").Socket): NodeStreamTransport {
    const transport = new NodeStreamTransport({ type: "socket" });
    transport._attach(socket);
    return transport;
  }

  /** Wire up event handlers for an already-connected socket. */
  private _attach(socket: import("net").Socket): void {
    this._socket = socket;
    this._connected = true;
    // A replaced socket keeps emitting until it drains
    const isCurrentSocket = (): boolean => this._socket === socket;
    socket.on("error", (err: Error) => {
      if (!isCurrentSocket()) return;
      this._closeHandler?.(err);
    });
    socket.on("data", (chunk: Buffer) => {
      if (!isCurrentSocket()) return;
      this._onData(chunk);
    });
    socket.on("close", (hadError: boolean) => {
      if (!isCurrentSocket()) return;
      this._connected = false;
      this._buffer = Buffer.alloc(0);
      const err = hadError ? new Error("socket closed with error") : undefined;
      this._closeHandler?.(err);
    });
  }

  /**
   * Dial the transport.
   *
   * Idempotent: a transport built by {@link fromSocket} is already connected,
   * and `SaikuroClient.open()` connects unconditionally.
   *
   * Throws when a socket adopted via {@link fromSocket} has gone away:
   * adoption carries no dial target, so there is nothing to reconnect to.
   */
  async connect(): Promise<void> {
    if (this._connected) return;
    const opts = this._connectionOptions;
    if (opts.type === "socket") {
      // Adoption carries no dial target, so there is nothing to reconnect to.
      throw new Error(
        "cannot reconnect a socket adopted via fromSocket, it has no dial target",
      );
    }

    const net = await import("net");
    return new Promise((resolve, reject) => {
      const connectArgs =
        opts.type === "tcp"
          ? { host: opts.host, port: opts.port }
          : { path: opts.path };

      const socket = net.createConnection(
        connectArgs as unknown as Parameters<typeof net.createConnection>[0],
        () => {
          socket.removeListener("error", onConnectError);
          this._attach(socket);
          resolve();
        },
      );

      const onConnectError = (err: Error) => reject(err);
      socket.on("error", onConnectError);
    });
  }

  /** Whether the transport holds a live socket. */
  get isConnected(): boolean {
    return this._connected;
  }

  private _onData(chunk: Buffer): void {
    this._buffer = Buffer.concat([this._buffer, chunk]);

    while (this._buffer.length >= 4) {
      const frameLen = this._buffer.readUInt32BE(0);
      if (frameLen > MAX_FRAME_SIZE) {
        this._socket?.destroy();
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
    this._connected = false;
    this._socket?.end();
  }

  async send(obj: object): Promise<void> {
    if (this._socket === undefined) throw new Error("not connected");
    if (!this._connected) throw new Error("not connected");
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
