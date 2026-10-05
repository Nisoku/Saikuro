import { getLogger } from "../logger";
import { Transport } from "./types";

const log = getLogger("saikuro.transport");

/**
 * Most messages a transport may hold for a not-yet-registered `onMessage`
 * handler.
 */
export const MAX_BUFFERED_MESSAGES = 64;

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

  /**
   * Messages received while no handler was registered, replayed to the first
   * handler that registers.
   */
  private readonly _undelivered: Record<string, unknown>[] = [];
  private _warnedAboutOverflow = false;

  abstract connect(): Promise<void>;
  abstract close(): Promise<void>;
  abstract send(obj: object): Promise<void>;
  abstract recv(): Promise<Record<string, unknown> | null>;

  onMessage(handler: (msg: Record<string, unknown>) => void): void {
    const isFirstHandler = this._messageHandlers.size === 0;
    this._messageHandlers.add(handler);

    if (isFirstHandler && this._undelivered.length > 0) {
      const buffered = this._undelivered.splice(0, this._undelivered.length);
      log.debug("replaying messages buffered before onMessage", {
        count: buffered.length,
      });
      for (const msg of buffered) handler(msg);
    }
  }

  offMessage(handler: (msg: Record<string, unknown>) => void): void {
    this._messageHandlers.delete(handler);
  }

  onClose(handler: (err?: Error) => void): void {
    this._closeHandler = handler;
  }

  /** Snapshot the current handler set and dispatch to each in registration order. */
  _dispatch(msg: Record<string, unknown>): void {
    if (this._messageHandlers.size === 0) {
      this._bufferOrDrop(msg);
      return;
    }
    for (const h of Array.from(this._messageHandlers)) h(msg);
  }

  /**
   * Hold `msg` for the next handler, or drop it once the buffer is full
   */
  private _bufferOrDrop(msg: Record<string, unknown>): void {
    if (this._undelivered.length >= MAX_BUFFERED_MESSAGES) {
      if (!this._warnedAboutOverflow) {
        this._warnedAboutOverflow = true;
        log.warn("dropping messages received before onMessage registered", {
          limit: MAX_BUFFERED_MESSAGES,
        });
      }
      return;
    }
    this._undelivered.push(msg);
  }
}
