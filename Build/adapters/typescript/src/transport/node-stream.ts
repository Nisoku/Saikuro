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
          socket.removeListener("error", onConnectError);
          socket.on("error", (err) => this._closeHandler?.(err));
          resolve();
        },
      );

      const onConnectError = (err: Error) => reject(err);
      socket.on("error", onConnectError);
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
    this._socket?.end();
  }

  async send(obj: object): Promise<void> {
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
