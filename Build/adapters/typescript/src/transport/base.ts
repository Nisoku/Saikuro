import { Transport } from "./types";

/**
 * Shared handler management for all transports.
 *
 * Every transport implementation needs the same `_messageHandlers` set,
 * `_closeHandler`, and the three registration methods.  This base class
 * provides them so each transport only writes the unique parts.
 */
export abstract class BaseTransport implements Transport {
  readonly _messageHandlers = new Set<(msg: Record<string, unknown>) => void>();
  _closeHandler?: (err?: Error) => void;

  abstract connect(): Promise<void>;
  abstract close(): Promise<void>;
  abstract send(obj: object): Promise<void>;
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
