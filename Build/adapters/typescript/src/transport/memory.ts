import { BaseTransport } from "./base";

//  In-Memory Transport

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

  async send(obj: object): Promise<void> {
    if (this._closed) throw new Error("transport is closed");
    const peer = this._peer;
    if (!peer || peer._closed) {
      throw new Error("peer transport is closed");
    }
    const msg = obj as Record<string, unknown>;
    peer._inbox.push(msg);
    peer._dispatch(msg);
  }

  async recv(): Promise<Record<string, unknown> | null> {
    return this._inbox.shift() ?? null;
  }
}
