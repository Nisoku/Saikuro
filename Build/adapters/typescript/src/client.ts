/**
 * Saikuro async client.
 *
 * The client manages one transport connection and multiplexes all call, cast,
 * stream, and channel invocations over it using invocation IDs as correlation
 * keys.
 *
 * Usage:
 *
 *   const client = await SaikuroClient.connect("unix:///tmp/saikuro.sock");
 *   const result = await client.call("math.add", [1, 2]);
 *   await client.close();
 *
 * Or as an async resource (using explicit open/close):
 *
 *   const client = SaikuroClient.fromTransport(transport);
 *   await client.open();
 *   // ...
 *   await client.close();
 */

import type { Transport } from "./transport";
import { makeTransport } from "./transport";
import {
  makeCallEnvelope,
  makeCastEnvelope,
  makeStreamOpenEnvelope,
  makeChannelOpenEnvelope,
  makeResourceEnvelope,
  makeBatchEnvelope,
  decodeResourceHandle,
  PROTOCOL_VERSION,
} from "./envelope";
import type { ResponseEnvelope, Envelope, ResourceHandle } from "./envelope";
import { SaikuroError } from "./error";
import type { ErrorPayload } from "./envelope";

// Stream / Channel handles

/**
 * Common base for stream/channel handles.
 *
 * Provides the buffered async-iterator plumbing used by both
 * [`SaikuroStream`] and [`SaikuroChannel`].
 */
class BaseSaikuroHandle<T = unknown>
  implements AsyncIterator<T>, AsyncIterable<T>
{
  protected readonly _id: string;
  protected _done = false;
  private readonly _buffer: Array<ResponseEnvelope | null> = [];
  private readonly _waiters: Array<(item: ResponseEnvelope | null) => void> = [];

  constructor(id: string) {
    this._id = id;
  }

  get invocationId(): string {
    return this._id;
  }

  /** @internal Called by the client's receive loop. */
  _deliver(resp: ResponseEnvelope): void {
    this._enqueue(resp);
  }

  /** @internal Called when the transport closes while still open. */
  _close(): void {
    this._done = true;
    for (const resolve of this._waiters.splice(0)) {
      this._buffer.push(null as unknown as ResponseEnvelope);
      resolve(null);
    }
  }

  private _enqueue(item: ResponseEnvelope | null): void {
    if (this._waiters.length > 0) {
      const resolve = this._waiters.shift()!;
      resolve(item);
    } else {
      this._buffer.push(item);
    }
  }

  private _take(): Promise<ResponseEnvelope | null> {
    if (this._buffer.length > 0) {
      return Promise.resolve(this._buffer.shift()!);
    }
    return new Promise<ResponseEnvelope | null>((resolve) => {
      this._waiters.push(resolve);
    });
  }

  async next(): Promise<IteratorResult<T>> {
    if (this._done) {
      return { done: true, value: undefined as unknown as T };
    }

    const item = await this._take();

    if (item === null) {
      this._done = true;
      return { done: true, value: undefined as unknown as T };
    }

    if (item.stream_control === "end") {
      this._done = true;
      return { done: true, value: undefined as unknown as T };
    }

    if (!item.ok) {
      this._done = true;
      const payload = (item.error ?? {
        code: "Internal",
        message: "stream ended with error",
      }) as ErrorPayload;
      throw SaikuroError.fromPayload(payload);
    }

    return { done: false, value: item.result as T };
  }

  [Symbol.asyncIterator](): AsyncIterator<T> {
    return this;
  }
}

/**
 * An async iterator that yields values from a server-to-client stream.
 *
 * Obtained from `client.stream(...)`.
 */
export class SaikuroStream<T = unknown> extends BaseSaikuroHandle<T> {}

/**
 * A bidirectional async channel.
 *
 * Obtained from `client.channel(...)`.
 */
export class SaikuroChannel<TIn = unknown, TOut = unknown>
  extends BaseSaikuroHandle<TIn>
{
  private readonly _sendFn: (id: string, value: unknown) => Promise<void>;

  /** @internal */
  constructor(
    id: string,
    sendFn: (id: string, value: unknown) => Promise<void>,
  ) {
    super(id);
    this._sendFn = sendFn;
  }

  /** Send a message to the provider side of the channel. */
  async send(value: TOut): Promise<void> {
    if (this._done) {
      throw new Error("channel is already closed");
    }
    await this._sendFn(this._id, value);
  }
}

// Client options

export interface ClientOptions {
  /**
   * Default timeout for `call` invocations, in milliseconds.
   * `0` means no timeout. Defaults to `0`.
   */
  defaultTimeoutMs?: number;
}

// SaikuroClient

/**
 * Async Saikuro client over a single transport connection.
 */
export class SaikuroClient {
  private readonly _transport: Transport;
  private readonly _options: Required<ClientOptions>;

  /** Pending call futures keyed by invocation ID. */
  private readonly _pendingCalls = new Map<
    string,
    {
      resolve: (resp: ResponseEnvelope) => void;
      reject: (err: unknown) => void;
      timer?: ReturnType<typeof setTimeout>;
    }
  >();

  /** Open streams keyed by invocation ID. */
  private readonly _openStreams = new Map<string, SaikuroStream<unknown>>();

  /** Open channels keyed by invocation ID. */
  private readonly _openChannels = new Map<
    string,
    SaikuroChannel<unknown, unknown>
  >();

  private _connected = false;

  private constructor(transport: Transport, options: ClientOptions = {}) {
    this._transport = transport;
    this._options = {
      defaultTimeoutMs: options.defaultTimeoutMs ?? 0,
    };
  }

  //  Factory

  /**
   * Connect to a Saikuro runtime at `address` and return a ready client.
   *
   * Address formats:
   *   - `unix:///path/to/socket`
   *   - `tcp://host:port`
   *   - `ws://host:port/path`  or  `wss://host:port/path`
   */
  static async connect(
    address: string,
    options?: ClientOptions,
  ): Promise<SaikuroClient> {
    const transport = makeTransport(address);
    return SaikuroClient.openOn(transport, options);
  }

  /** Construct a client from an already-instantiated transport and connect it. */
  static async openOn(
    transport: Transport,
    options?: ClientOptions,
  ): Promise<SaikuroClient> {
    const client = new SaikuroClient(transport, options);
    await client.open();
    return client;
  }

  /** Construct a client from a transport without connecting immediately. */
  static fromTransport(
    transport: Transport,
    options?: ClientOptions,
  ): SaikuroClient {
    return new SaikuroClient(transport, options);
  }

  //  Lifecycle

  /** Connect the transport and start the receive loop. */
  async open(): Promise<void> {
    await this._transport.connect();
    this._connected = true;

    this._transport.onMessage((raw) => this._handleRaw(raw));
    this._transport.onClose((err) => this._handleClose(err));
  }

  /** Returns `true` if the client is currently connected. */
  get connected(): boolean {
    return this._connected;
  }

  /** Gracefully close the client and its transport. */
  async close(): Promise<void> {
    this._connected = false;
    await this._transport.close();
    this._teardownPending(new Error("client closed"));
  }

  //  Invocation API

  /**
   * Perform a request/response call and return the result value.
   *
   * @throws {SaikuroError} (or a specific subclass) on a wire-level error.
   * @throws {Error} on timeout when `timeoutMs` or `defaultTimeoutMs` is set.
   */
  async call(
    target: string,
    args: readonly unknown[],
    options?: {
      capability?: string;
      timeoutMs?: number;
    },
  ): Promise<unknown> {
    const envelope = makeCallEnvelope(target, args, options?.capability);
    const timeoutMs = options?.timeoutMs ?? this._options.defaultTimeoutMs;

    const response = await this._sendAndWait(envelope, timeoutMs);

    if (!response.ok) {
      const payload = (response.error ?? {
        code: "Internal",
        message: "call failed",
      }) as ErrorPayload;
      throw SaikuroError.fromPayload(payload);
    }
    return response.result;
  }

  /**
   * Fire-and-forget invocation. No response is expected.
   */
  async cast(
    target: string,
    args: readonly unknown[],
    options?: { capability?: string },
  ): Promise<void> {
    const envelope = makeCastEnvelope(target, args, options?.capability);
    await this._transport.send(envelope as unknown as Record<string, unknown>);
  }

  /**
   * Invoke a provider function that manages an external resource and return
   * the resulting {@link ResourceHandle}.
   *
   * Throws a `SaikuroError` if the invocation fails or the result is not a
   * valid handle.
   *
   * @example
   * ```ts
   * const handle = await client.resource("files.open", ["/var/data/report.csv"]);
   * console.log(handle.id, handle.mime_type, handle.size, handle.uri);
   * ```
   */
  async resource(
    target: string,
    args: readonly unknown[],
    options?: {
      capability?: string;
      timeoutMs?: number;
    },
  ): Promise<ResourceHandle> {
    const envelope = makeResourceEnvelope(target, args, options?.capability);
    const timeoutMs = options?.timeoutMs ?? this._options.defaultTimeoutMs;

    const response = await this._sendAndWait(envelope, timeoutMs);

    if (!response.ok) {
      const payload = (response.error ?? {
        code: "Internal",
        message: "resource call failed",
      }) as ErrorPayload;
      throw SaikuroError.fromPayload(payload);
    }

    const handle = decodeResourceHandle(response.result);
    if (handle === null) {
      throw SaikuroError.fromPayload({
        code: "ProviderError",
        message: `resource invocation for "${target}" returned an invalid or missing ResourceHandle`,
      });
    }
    return handle;
  }

  /**
   * Forward a structured log record to the runtime log sink.
   *
   * Fire-and-forget; no response is expected.
   * For automatic forwarding of all `Logger` calls, use
   * `setLogSink(createTransportSink(client))` from `"saikuro"`.
   */
  async log(
    level: "trace" | "debug" | "info" | "warn" | "error",
    name: string,
    msg: string,
    fields?: Record<string, unknown>,
  ): Promise<void> {
    const ts = new Date().toISOString();
    const logRecord: Record<string, unknown> = { ts, level, name, msg };
    if (fields !== undefined && Object.keys(fields).length > 0) {
      logRecord["fields"] = fields;
    }
    const envelope: Record<string, unknown> = {
      version: PROTOCOL_VERSION,
      type: "log",
      id: `log-${ts}`,
      target: "$log",
      args: [logRecord],
    };
    await this._transport.send(envelope);
  }

  /**
   * Execute multiple calls in a single round-trip.
   *
   * Returns an ordered array of per-call results. Throws a `SaikuroError`
   * only if the batch envelope itself is rejected.
   *
   * @example
   * ```ts
   * const [sum, product] = await client.batch([
   *   { target: "math.add",      args: [1, 2] },
   *   { target: "math.multiply", args: [3, 4] },
   * ]);
   * ```
   */
  async batch(
    calls: ReadonlyArray<{
      target: string;
      args: readonly unknown[];
      capability?: string;
    }>,
    options?: { timeoutMs?: number },
  ): Promise<unknown[]> {
    const items = calls.map(({ target, args, capability }) =>
      makeCallEnvelope(target, args, capability),
    );
    const batchEnvelope = makeBatchEnvelope(items);
    const timeoutMs = options?.timeoutMs ?? this._options.defaultTimeoutMs;

    const response = await this._sendAndWait(batchEnvelope, timeoutMs);

    if (!response.ok) {
      const payload = (response.error ?? {
        code: "Internal",
        message: "batch call failed",
      }) as ErrorPayload;
      throw SaikuroError.fromPayload(payload);
    }

    // The result should be an array; if the runtime returns a single value
    // (e.g. for a single-item batch optimisation), wrap it for consistency.
    if (Array.isArray(response.result)) {
      return response.result as unknown[];
    }
    return [response.result];
  }

  /**
   * Open a server-to-client stream. Returns an async iterable that yields
   * values as they arrive from the provider.
   */
  async stream<T = unknown>(
    target: string,
    args: readonly unknown[],
    options?: { capability?: string },
  ): Promise<SaikuroStream<T>> {
    const envelope = makeStreamOpenEnvelope(target, args);
    const patched: Envelope = {
      ...envelope,
      ...(options?.capability !== undefined && {
        capability: options.capability,
      }),
    };
    const streamHandle = new SaikuroStream<T>(patched.id);
    this._openStreams.set(patched.id, streamHandle as SaikuroStream<unknown>);
    await this._transport.send(patched as unknown as Record<string, unknown>);
    return streamHandle;
  }

  /**
   * Open a bidirectional channel. Returns a `SaikuroChannel` which is both an
   * async iterable (receive) and has a `send` method (transmit).
   */
  async channel<TIn = unknown, TOut = unknown>(
    target: string,
    args: readonly unknown[],
    options?: { capability?: string },
  ): Promise<SaikuroChannel<TIn, TOut>> {
    const envelope = makeChannelOpenEnvelope(target, args);
    const patched: Envelope = {
      ...envelope,
      ...(options?.capability !== undefined && {
        capability: options.capability,
      }),
    };
    const channelHandle = new SaikuroChannel<TIn, TOut>(
      patched.id,
      (id, value) => this._channelSend(id, value),
    );
    this._openChannels.set(
      patched.id,
      channelHandle as SaikuroChannel<unknown, unknown>,
    );
    await this._transport.send(patched as unknown as Record<string, unknown>);
    return channelHandle;
  }

  //  Internal

  private async _sendAndWait(
    envelope: Envelope,
    timeoutMs: number,
  ): Promise<ResponseEnvelope> {
    return new Promise<ResponseEnvelope>((resolve, reject) => {
      let timer: ReturnType<typeof setTimeout> | undefined;

      if (timeoutMs > 0) {
        timer = setTimeout(() => {
          this._pendingCalls.delete(envelope.id);
          reject(
            new Error(
              `call to "${envelope.target}" timed out after ${timeoutMs}ms`,
            ),
          );
        }, timeoutMs);
      }

      this._pendingCalls.set(envelope.id, {
        resolve,
        reject,
        ...(timer !== undefined && { timer }),
      });
      this._transport
        .send(envelope as unknown as Record<string, unknown>)
        .catch((err) => {
          this._pendingCalls.delete(envelope.id);
          if (timer !== undefined) clearTimeout(timer);
          reject(err);
        });
    });
  }

  private async _channelSend(channelId: string, value: unknown): Promise<void> {
    const envelope: Envelope = {
      version: PROTOCOL_VERSION,
      type: "channel",
      id: channelId,
      target: "",
      args: [value],
    };
    await this._transport.send(envelope as unknown as Record<string, unknown>);
  }

  private _handleRaw(raw: Record<string, unknown>): void {
    // Decode the response envelope from the raw msgpack map.
    // Build a mutable scratch object then freeze it as ResponseEnvelope so
    // that exactOptionalPropertyTypes is satisfied (no `key: undefined`).
    const errorField = raw["error"] as ErrorPayload | undefined;
    const seqField = raw["seq"] as number | undefined;
    const scField = raw["stream_control"] as
      | ResponseEnvelope["stream_control"]
      | undefined;

    const scratch: Record<string, unknown> = {
      id: raw["id"] as string,
      ok: raw["ok"] as boolean,
      result: raw["result"],
    };
    if (errorField !== undefined) scratch["error"] = errorField;
    if (seqField !== undefined) scratch["seq"] = seqField;
    if (scField !== undefined) scratch["stream_control"] = scField;

    this._dispatchResponse(scratch as unknown as ResponseEnvelope);
  }

  private _dispatchResponse(resp: ResponseEnvelope): void {
    const id = resp.id;

    // Pending call?
    const pending = this._pendingCalls.get(id);
    if (pending !== undefined) {
      this._pendingCalls.delete(id);
      if (pending.timer !== undefined) clearTimeout(pending.timer);
      pending.resolve(resp);
      return;
    }

    // Open stream?
    const stream = this._openStreams.get(id);
    if (stream !== undefined) {
      stream._deliver(resp);
      if (
        resp.stream_control === "end" ||
        resp.stream_control === "abort" ||
        !resp.ok
      ) {
        this._openStreams.delete(id);
      }
      return;
    }

    // Open channel?
    const channel = this._openChannels.get(id);
    if (channel !== undefined) {
      channel._deliver(resp);
      if (
        resp.stream_control === "end" ||
        resp.stream_control === "abort" ||
        !resp.ok
      ) {
        this._openChannels.delete(id);
      }
      return;
    }

    // Unknown: silently ignore (could be a late response after timeout).
  }

  private _handleClose(err?: Error): void {
    this._connected = false;
    this._teardownPending(err ?? new Error("transport closed unexpectedly"));
  }

  private _teardownPending(err: Error): void {
    const transportPayload: ErrorPayload = {
      code: "ConnectionLost",
      message: err.message,
    };

    for (const [, pending] of this._pendingCalls) {
      if (pending.timer !== undefined) clearTimeout(pending.timer);
      pending.reject(SaikuroError.fromPayload(transportPayload));
    }
    this._pendingCalls.clear();

    for (const [, stream] of this._openStreams) {
      stream._close();
    }
    this._openStreams.clear();

    for (const [, channel] of this._openChannels) {
      channel._close();
    }
    this._openChannels.clear();
  }
}
