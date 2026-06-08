import { encode, decode } from "@msgpack/msgpack";
import { getLogger } from "../logger";
import { BaseTransport } from "./base";

const log = getLogger("saikuro.transport");

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

  async send(obj: object): Promise<void> {
    if (this._ws === undefined || this._ws.readyState !== this._WS.OPEN) {
      throw new Error("WebSocket is not connected");
    }
    const payload = encode(obj);
    this._ws.send(payload);
  }

  async recv(): Promise<Record<string, unknown> | null> {
    return null;
  }
}
