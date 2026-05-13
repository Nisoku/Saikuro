/**
 * Saikuro provider: register TypeScript functions and serve them to the runtime.
 *
 * A provider:
 *   1. Connects to a Saikuro runtime via a transport.
 *   2. Optionally announces its schema via a special envelope.
 *   3. Listens for inbound invocation envelopes.
 *   4. Dispatches to the matching registered handler.
 *   5. Sends back response envelopes (including stream items for async generators).
 *
 */

import { makeTransport } from "./transport";
import type { Transport } from "./transport";
import { PROTOCOL_VERSION, makeAnnounceEnvelope } from "./envelope";
import { extractSchema } from "./schema_extractor";
import type {
  Envelope,
  SaikuroSchema,
  FunctionSchema as WireFunctionSchema,
} from "./envelope";
import { SaikuroError } from "./error";
import { getLogger } from "./logger";

const log = getLogger("saikuro.provider");

// Handler types

/** A synchronous or async function that can be registered as a handler. */
export type Handler = (...args: unknown[]) => unknown | Promise<unknown>;

/** An async generator function used for stream-returning handlers. */
export type StreamHandler = (...args: unknown[]) => AsyncGenerator<unknown>;

export type AnyHandler = Handler | StreamHandler;

// TypeDescriptor

/**
 * Mirrors the Saikuro `TypeDescriptor` wire format.
 *
 * Use the `t.*` builder helpers to construct these without typos.
 */
export type TypeDescriptor =
  | {
      readonly kind: "primitive";
      readonly type:
        | "bool"
        | "i32"
        | "i64"
        | "f32"
        | "f64"
        | "string"
        | "bytes"
        | "any"
        | "unit";
    }
  | { readonly kind: "list"; readonly item: TypeDescriptor }
  | {
      readonly kind: "map";
      readonly key: TypeDescriptor;
      readonly value: TypeDescriptor;
    }
  | { readonly kind: "optional"; readonly inner: TypeDescriptor }
  | { readonly kind: "named"; readonly name: string }
  | { readonly kind: "stream"; readonly item: TypeDescriptor }
  | {
      readonly kind: "channel";
      readonly send: TypeDescriptor;
      readonly recv: TypeDescriptor;
    };

/**
 * Builder helpers for constructing `TypeDescriptor` values.
 *
 * Example:
 *   t.string()          -> { kind: "primitive", type: "string" }
 *   t.list(t.i32())     -> { kind: "list", item: { kind: "primitive", type: "i32" } }
 *   t.optional(t.bool()) -> { kind: "optional", inner: { kind: "primitive", type: "bool" } }
 */
export const t = {
  bool: (): TypeDescriptor => ({ kind: "primitive", type: "bool" }),
  i32: (): TypeDescriptor => ({ kind: "primitive", type: "i32" }),
  i64: (): TypeDescriptor => ({ kind: "primitive", type: "i64" }),
  f32: (): TypeDescriptor => ({ kind: "primitive", type: "f32" }),
  f64: (): TypeDescriptor => ({ kind: "primitive", type: "f64" }),
  string: (): TypeDescriptor => ({ kind: "primitive", type: "string" }),
  bytes: (): TypeDescriptor => ({ kind: "primitive", type: "bytes" }),
  any: (): TypeDescriptor => ({ kind: "primitive", type: "any" }),
  unit: (): TypeDescriptor => ({ kind: "primitive", type: "unit" }),
  list: (item: TypeDescriptor): TypeDescriptor => ({ kind: "list", item }),
  map: (key: TypeDescriptor, value: TypeDescriptor): TypeDescriptor => ({
    kind: "map",
    key,
    value,
  }),
  optional: (inner: TypeDescriptor): TypeDescriptor => ({
    kind: "optional",
    inner,
  }),
  named: (name: string): TypeDescriptor => ({ kind: "named", name }),
  stream: (item: TypeDescriptor): TypeDescriptor => ({ kind: "stream", item }),
  channel: (send: TypeDescriptor, recv: TypeDescriptor): TypeDescriptor => ({
    kind: "channel",
    send,
    recv,
  }),
} as const;

// Schema types (minimal, for announcement)

/**
 * A single argument descriptor as stored during `register()`.
 *
 * `type` mirrors the Saikuro `TypeDescriptor` wire format.
 * When not supplied, `{ kind: "primitive", type: "any" }` is used as a fallback
 * during schema emission.
 */
export interface ArgDescriptor {
  readonly name: string;
  readonly type?: TypeDescriptor;
  readonly optional?: boolean;
  readonly default?: unknown;
  readonly doc?: string;
}

export interface FunctionSchema {
  readonly name: string;
  readonly doc?: string;
  readonly capabilities?: readonly string[];
  readonly idempotent?: boolean;
  readonly visibility?: "public" | "internal" | "private";
  /** Wire-format argument descriptors stored at `register()` time. */
  readonly args?: readonly ArgDescriptor[];
  /** Wire-format return type descriptor stored at `register()` time. */
  readonly returns?: TypeDescriptor;
}

// SaikuroProvider

/**
 * A Saikuro provider that exposes TypeScript/JavaScript functions within a
 * single namespace.
 */
export class SaikuroProvider {
  private readonly _namespace: string;
  private readonly _handlers = new Map<string, AnyHandler>();
  private readonly _schema = new Map<string, FunctionSchema>();

  constructor(namespace: string) {
    this._namespace = namespace;
  }

  get namespace(): string {
    return this._namespace;
  }

  // Registration

  /**
   * Register a function under the given local `name` within this provider's
   * namespace.  The full qualified name becomes `"<namespace>.<name>"`.
   *
   * Works with sync functions, async functions, and async generator functions
   * (which will produce server-to-client streams).
   */
  register(
    name: string,
    handler: AnyHandler,
    options?: {
      capabilities?: readonly string[];
      doc?: string;
      args?: readonly ArgDescriptor[];
      returns?: TypeDescriptor;
      idempotent?: boolean;
      visibility?: "public" | "internal" | "private";
    },
  ): this {
    this._handlers.set(name, handler);
    const schema: Record<string, unknown> = { name };
    if (options?.doc !== undefined) schema["doc"] = options.doc;
    if (options?.capabilities !== undefined)
      schema["capabilities"] = options.capabilities;
    if (options?.args !== undefined) schema["args"] = options.args;
    if (options?.returns !== undefined) schema["returns"] = options.returns;
    if (options?.idempotent !== undefined)
      schema["idempotent"] = options.idempotent;
    if (options?.visibility !== undefined)
      schema["visibility"] = options.visibility;
    this._schema.set(name, schema as unknown as FunctionSchema);
    return this;
  }

  /**
   * Decorator-style registration.  Returns a function that registers the
   * decorated handler and then returns it unchanged.
   *
   * Usage:
   *   @provider.decorator("greet")
   *   async function greet(name: string) { return `Hello, ${name}`; }
   */
  decorator(
    name: string,
    options?: {
      capabilities?: readonly string[];
      doc?: string;
      args?: readonly ArgDescriptor[];
      returns?: TypeDescriptor;
      idempotent?: boolean;
      visibility?: "public" | "internal" | "private";
    },
  ): <F extends AnyHandler>(fn: F) => F {
    return <F extends AnyHandler>(fn: F): F => {
      this.register(name, fn, options);
      return fn;
    };
  }

  // Schema

  /**
   * Return the namespace schema as a plain {@link SaikuroSchema} object
   * suitable for schema announcement.
   *
   * TypeScript doesn't carry runtime type information, so all arguments default
   * to `any` and return types to `unit`. Use the code-generator for typed
   * schemas.
   */
  schemaObject(): SaikuroSchema {
    const functions: Record<string, WireFunctionSchema> = {};
    for (const [name, meta] of this._schema.entries()) {
      // Map stored ArgDescriptors to wire ArgumentDescriptors.
      // Fall back to [] when no args were provided at register() time.
      const wireArgs: WireFunctionSchema["args"] =
        meta.args !== undefined
          ? meta.args.map((a) => {
              const wireArg: Record<string, unknown> = {
                name: a.name,
                type: a.type ?? { kind: "primitive", type: "any" },
              };
              if (a.optional !== undefined) wireArg["optional"] = a.optional;
              if (a.default !== undefined) wireArg["default"] = a.default;
              if (a.doc !== undefined) wireArg["doc"] = a.doc;
              return wireArg as unknown as import("./envelope").ArgumentDescriptor;
            })
          : [];

      // Fall back to "any" when no returns was provided.
      const wireReturns: unknown = meta.returns ?? {
        kind: "primitive",
        type: "any",
      };

      const fn: WireFunctionSchema = {
        args: wireArgs,
        returns: wireReturns,
        visibility: meta.visibility ?? "public",
        capabilities: meta.capabilities ?? [],
        ...(meta.doc !== undefined && { doc: meta.doc }),
        ...(meta.idempotent !== undefined && { idempotent: meta.idempotent }),
      };
      functions[name] = fn;
    }
    return {
      version: 1,
      namespaces: {
        [this._namespace]: {
          functions,
        },
      },
      types: {},
    };
  }

  // Dispatch

  /**
   * Dispatch a single inbound invocation envelope.
   *
   * Called by the serve loop and also usable in testing.
   */
  async dispatch(envelope: Envelope, transport: Transport): Promise<void> {
    if (envelope.type === "batch") {
      await this._dispatchBatch(envelope, transport);
      return;
    }

    const isCast = envelope.type === "cast";

    // Extract the local function name (last segment of "namespace.fn_name").
    const fnName = envelope.target.includes(".")
      ? envelope.target.slice(envelope.target.lastIndexOf(".") + 1)
      : envelope.target;

    const handler = this._handlers.get(fnName);
    if (handler === undefined) {
      if (!isCast) {
        await _sendError(
          transport,
          envelope.id,
          "FunctionNotFound",
          `no handler registered for '${envelope.target}'`,
        );
      }
      return;
    }

    try {
      const result = handler(...(envelope.args as unknown[]));

      // Async generator -> stream
      if (_isAsyncGenerator(result)) {
        await this._dispatchStream(
          envelope,
          result as AsyncGenerator<unknown>,
          transport,
        );
        return;
      }

      // Regular async or sync result
      const resolved = await Promise.resolve(result);
      if (!isCast) {
        await _sendOk(transport, envelope.id, resolved);
      }
    } catch (err) {
      if (isCast) return;
      if (err instanceof SaikuroError) {
        await _sendError(
          transport,
          envelope.id,
          err.code,
          err.message,
          err.details,
        );
      } else {
        await _sendError(
          transport,
          envelope.id,
          "ProviderError",
          err instanceof Error ? err.message : String(err),
        );
      }
    }
  }

  /**
   * Handle a `batch` envelope by dispatching each item individually and
   * returning the results as a single ordered array.
   *
   * If any item fails, the whole batch fails and the error includes the
   * failing index and target in `details`.
   */
  private async _dispatchBatch(
    envelope: Envelope,
    transport: Transport,
  ): Promise<void> {
    const items = (envelope.batch_items ?? []) as Envelope[];

    try {
      const results: unknown[] = [];

      for (let i = 0; i < items.length; i++) {
        const item = items[i];
        let captured: unknown = null;
        let capturedError: Record<string, unknown> | null = null;

        // A minimal in-process sink that records the response from dispatch.
        const sink: Transport = {
          connect: async () => {},
          close: async () => {},
          send: async (obj: Record<string, unknown>): Promise<void> => {
            if (obj["ok"] === true) {
              captured = obj["result"] ?? null;
            } else {
              capturedError = obj["error"] as Record<string, unknown>;
            }
          },
          recv: async () => null,
          onMessage: () => {},
          offMessage: () => {},
          onClose: () => {},
        };

        // Rewrite type to "call" so the handler path runs normally.
        const callEnvelope: Envelope = {
          version: item.version,
          type: "call",
          id: item.id,
          target: item.target,
          args: item.args,
          ...(item.capability !== undefined && { capability: item.capability }),
          ...(item.meta !== undefined && { meta: item.meta }),
        };

        await this.dispatch(callEnvelope, sink);

        if (capturedError !== null) {
          const errCode = (capturedError["code"] as string) ?? "ProviderError";
          const errMsg =
            (capturedError["message"] as string) ?? "unknown error";
          await _sendError(transport, envelope.id, errCode, errMsg, {
            batch_index: i,
            target: item.target,
          });
          return;
        }

        results.push(captured);
      }

      await _sendOk(transport, envelope.id, results);
    } catch (err) {
      await _sendError(
        transport,
        envelope.id,
        "ProviderError",
        err instanceof Error ? err.message : String(err),
      );
    }
  }

  private async _dispatchStream(
    envelope: Envelope,
    gen: AsyncGenerator<unknown>,
    transport: Transport,
  ): Promise<void> {
    let seq = 0;
    try {
      for await (const item of gen) {
        await transport.send({
          id: envelope.id,
          ok: true,
          result: item,
          seq,
        });
        seq++;
      }
      // End-of-stream sentinel.
      await transport.send({
        id: envelope.id,
        ok: true,
        seq,
        stream_control: "end",
      });
    } catch (err) {
      // Emit an error response followed by an abort control.
      await _sendError(
        transport,
        envelope.id,
        "ProviderError",
        err instanceof Error ? err.message : String(err),
      );
      await transport.send({
        id: envelope.id,
        ok: false,
        seq,
        stream_control: "abort",
      });
    }
  }

  // Server

  /**
   * Connect to the runtime at `address` and begin serving invocations.
   * This call runs until the transport is closed.
   */
  async serve(address: string): Promise<void> {
    const transport = makeTransport(address);
    await transport.connect();
    await this._announce(transport);
    await this._runServeLoop(transport);
    await transport.close();
  }

  /**
   * Serve invocations on an already-connected transport.
   * This call runs until the transport is closed.
   */
  async serveOn(
    transport: Transport,
    options?: { dev?: boolean; sourceFiles?: string[] },
  ): Promise<void> {
    if (options?.dev && Array.isArray(options.sourceFiles)) {
      try {
        const schema = await extractSchema(
          options.sourceFiles,
          this._namespace,
        );
        await this._announce(transport, schema as SaikuroSchema);
      } catch (err) {
        // If extraction fails, fallback to regular announce behavior.
        log.warn("dev schema extraction failed, falling back to built schema", {
          err: err instanceof Error ? err.message : String(err),
        });
        await this._announce(transport);
      }
    } else {
      await this._announce(transport);
    }

    await this._runServeLoop(transport);
  }

  private _runServeLoop(transport: Transport): Promise<void> {
    return new Promise<void>((resolve) => {
      transport.onClose(() => resolve());

      transport.onMessage((raw) => {
        const envelope = _rawToEnvelope(raw);
        if (envelope === null) {
          // Already logged inside _rawToEnvelope.
          return;
        }
        // Fire dispatch in the background; unhandled rejections are logged here
        // so no exception is silently swallowed.
        this.dispatch(envelope, transport).catch((err: unknown) => {
          log.error("unhandled exception in dispatch", {
            target: envelope.target,
            id: envelope.id,
            err: err instanceof Error ? err.message : String(err),
          });
        });
      });
    });
  }

  /**
   * Send a schema-announcement envelope and wait for the runtime's ack.
   * Any failure is logged but does not abort the serve loop.
   */
  private async _announce(
    transport: Transport,
    schemaOverride?: SaikuroSchema,
  ): Promise<void> {
    try {
      const schema = schemaOverride ?? this.schemaObject();
      const envelope = makeAnnounceEnvelope(schema);
      // Register a one-shot listener for the ack before sending so we don't
      // miss an immediate response from an in-memory transport.
      let tid: ReturnType<typeof setTimeout> | null = null;
      let onMsg: ((raw: Record<string, unknown>) => void) | null = null;
      const ackPromise = new Promise<Record<string, unknown> | null>((res) => {
        tid = setTimeout(() => {
          log.warn("schema announce: timed out waiting for ack");
          // Clean up listener on timeout
          if (onMsg) transport.offMessage(onMsg);
          res(null);
        }, 5000);
        onMsg = (raw: Record<string, unknown>): void => {
          if (tid)
            clearTimeout(tid as unknown as ReturnType<typeof setTimeout>);
          if (onMsg) transport.offMessage(onMsg);
          res(raw);
        };
        transport.onMessage(onMsg);
      });

      try {
        await transport.send(envelope as unknown as Record<string, unknown>);
      } catch (err) {
        // Sending failed; remove listener and rethrow.
        if (onMsg) transport.offMessage(onMsg);
        if (tid) clearTimeout(tid as unknown as ReturnType<typeof setTimeout>);
        throw err;
      }

      const ack = await ackPromise;

      if (ack !== null) {
        if (ack["ok"] === true) {
          log.debug("schema announce acknowledged");
        } else {
          log.warn("schema announce rejected by runtime", {
            error: ack["error"],
          });
        }
      }
    } catch (err) {
      log.warn("schema announce failed (continuing anyway)", {
        err: err instanceof Error ? err.message : String(err),
      });
    }
  }
}

// Helpers

function _isAsyncGenerator(value: unknown): boolean {
  if (value == null || typeof value !== "object") return false;
  return Symbol.asyncIterator in value;
}

function _rawToEnvelope(raw: Record<string, unknown>): Envelope | null {
  try {
    // Build with required fields first, then assign optional fields only when
    // present: required by exactOptionalPropertyTypes.
    const scratch: Record<string, unknown> = {
      version: (raw["version"] as number) ?? PROTOCOL_VERSION,
      type: raw["type"] as Envelope["type"],
      id: raw["id"] as string,
      target: raw["target"] as string,
      args: (raw["args"] as unknown[]) ?? [],
    };
    if (raw["meta"] !== undefined) scratch["meta"] = raw["meta"];
    if (raw["capability"] !== undefined)
      scratch["capability"] = raw["capability"];
    if (raw["batch_items"] !== undefined)
      scratch["batch_items"] = raw["batch_items"];
    if (raw["stream_control"] !== undefined)
      scratch["stream_control"] = raw["stream_control"];
    if (raw["seq"] !== undefined) scratch["seq"] = raw["seq"];

    const envelope = scratch as unknown as Envelope;
    // Validate mandatory fields so callers can rely on them being present.
    if (typeof envelope.id !== "string" || envelope.id.length === 0) {
      throw new TypeError(
        `missing or invalid 'id' field: ${JSON.stringify(raw["id"])}`,
      );
    }
    // `target` may be empty for batch envelopes; only reject a missing field.
    if (typeof envelope.target !== "string") {
      throw new TypeError(
        `missing or invalid 'target' field: ${JSON.stringify(raw["target"])}`,
      );
    }
    return envelope;
  } catch (err) {
    log.error("malformed inbound envelope, skipping", {
      err: err instanceof Error ? err.message : String(err),
      raw: JSON.stringify(raw),
    });
    return null;
  }
}

async function _sendOk(
  transport: Transport,
  id: string,
  result: unknown,
): Promise<void> {
  await transport.send({ id, ok: true, result });
}

async function _sendError(
  transport: Transport,
  id: string,
  code: string,
  message: string,
  details?: Readonly<Record<string, unknown>>,
): Promise<void> {
  const error: Record<string, unknown> = { code, message };
  if (details !== undefined && Object.keys(details).length > 0) {
    error["details"] = details;
  }
  await transport.send({ id, ok: false, error });
}
