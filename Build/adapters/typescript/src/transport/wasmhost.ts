import { encode, decode } from "@msgpack/msgpack";
import { getLogger } from "../logger";
import { BaseTransport } from "./base";

const log = getLogger("saikuro.transport");

function generateShortId(): string {
  const now = Date.now() & 0xffffffff;
  const rand = Math.floor(Math.random() * 0x100000000);
  return `${now.toString(16).padStart(8, "0")}${rand.toString(16).padStart(8, "0")}`;
}

export class WasmHostTransport extends BaseTransport {
  private _channel: BroadcastChannel | undefined = undefined;
  private readonly _baseChannel: string;
  private _connected = false;
  private _closed = false;

  constructor(baseChannel: string) {
    super();
    this._baseChannel = baseChannel;
  }

  static fromChannel(channel: BroadcastChannel): WasmHostTransport {
    const t = new WasmHostTransport(channel.name);
    t._channel = channel;
    t._connected = true;
    t._setupMessageHandler();
    return t;
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
        log.debug("wasmhost recv", {
          type: (msg.type as string) ?? "msg",
          target: (msg.target as string) ?? "-",
        });
        this._dispatch(msg);
      } catch (err) {
        log.error("wasmhost channel frame decode error", {
          err: String(err),
        });
      }
    };
  }

  connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      if (this._connected) {
        resolve();
        return;
      }

      const connId = generateShortId();
      const privateName = `${this._baseChannel}:${connId}`;
      const privateChannel = new BroadcastChannel(privateName);

      log.debug("wasmhost connect handshake start", {
        connId,
        baseChannel: this._baseChannel,
        privateName,
      });

      const timeoutId = setTimeout(() => {
        privateChannel.removeEventListener("message", acceptHandler);
        privateChannel.close();
        log.warn("wasmhost connect timeout", { connId });
        reject(new Error("WasmHostTransport: connect timeout"));
      }, 10000);

      const acceptHandler = (event: MessageEvent) => {
        const data = event.data as Record<string, unknown> | undefined;
        if (data?.type !== "accept" || data?.id !== connId) return;
        clearTimeout(timeoutId);
        privateChannel.removeEventListener("message", acceptHandler);
        this._channel = privateChannel;
        this._connected = true;
        this._setupMessageHandler();
        log.info("wasmhost connect accepted", { connId, privateName });
        resolve();
      };
      privateChannel.addEventListener("message", acceptHandler);

      const baseChannel = new BroadcastChannel(this._baseChannel);
      baseChannel.postMessage({ type: "connect", id: connId });
      baseChannel.close();
    });
  }

  async close(): Promise<void> {
    if (this._closed) return;
    log.debug("wasmhost close");
    this._closed = true;
    this._connected = false;
    this._channel?.close();
    this._channel = undefined;
    this._closeHandler?.();
  }

  async send(obj: object): Promise<void> {
    const ch = this._channel;
    if (!this._connected || !ch) {
      throw new Error("WasmHostTransport: not connected");
    }
    const payload = encode(obj);
    const buffer = payload.buffer.slice(
      payload.byteOffset,
      payload.byteOffset + payload.byteLength,
    );
    log.debug("wasmhost send", {
      type: (obj as Record<string, string>).type ?? "msg",
    });
    ch.postMessage(buffer);
  }

  async recv(): Promise<Record<string, unknown> | null> {
    return null;
  }
}

export class WasmHostConnector {
  private readonly _channelName: string;

  constructor(channelName: string) {
    this._channelName = channelName;
  }

  connect(): Promise<WasmHostTransport> {
    const t = new WasmHostTransport(this._channelName);
    return t.connect().then(() => t);
  }
}

export class WasmHostListener {
  private readonly _baseChannel: BroadcastChannel;
  private readonly _connectQueue: Array<{
    resolve: (t: WasmHostTransport) => void;
    reject: (err: Error) => void;
  }> = [];
  private readonly _pendingConnections: WasmHostTransport[] = [];
  private _closed = false;

  constructor(channelName: string) {
    this._baseChannel = new BroadcastChannel(channelName);
    log.debug("wasmhost listener created", { channelName });
    this._baseChannel.onmessage = (ev: MessageEvent) => {
      if (this._closed) return;
      const data = ev.data as { type?: string; id?: string };
      if (data.type === "connect" && data.id) {
        log.debug("wasmhost listener got connect request", {
          connId: data.id,
          channelName,
        });
        const privateName = `${channelName}:${data.id}`;
        const privateChannel = new BroadcastChannel(privateName);

        privateChannel.postMessage({ type: "accept", id: data.id });
        log.debug("wasmhost listener sent accept", {
          connId: data.id,
          privateName,
        });

        const transport = WasmHostTransport.fromChannel(privateChannel);

        const queued = this._connectQueue.shift();
        if (queued) {
          queued.resolve(transport);
        } else {
          this._pendingConnections.push(transport);
        }
      }
    };
  }

  accept(): Promise<WasmHostTransport> {
    return new Promise((resolve, reject) => {
      if (this._closed) {
        reject(new Error("WasmHostListener is closed"));
        return;
      }
      const pending = this._pendingConnections.shift();
      if (pending) {
        resolve(pending);
      } else {
        this._connectQueue.push({ resolve, reject });
      }
    });
  }

  async close(): Promise<void> {
    if (this._closed) return;
    this._closed = true;
    this._baseChannel.close();
    for (const queued of this._connectQueue.splice(0)) {
      queued.reject(new Error("WasmHostListener is closed"));
    }
    for (const transport of this._pendingConnections.splice(0)) {
      await transport.close();
    }
  }
}
