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
import {
  PROTOCOL_VERSION,
  makeAnnounceEnvelope,
  makeSchemaObject,
} from "./envelope";
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

// _CaptureSink

/**
 * Minimal in-process Transport sink that captures a single response from
 * dispatch. Used in `_dispatchBatch` to collect per-item results.
 */
class _CaptureSink implements Transport {
  captured: unknown = null;
  capturedError: Record<string, unknown> | null = null;

  async connect(): Promise<void> {}
  async close(): Promise<void> {}
  async send(obj: Record<string, unknown>): Promise<void> {
    if (obj["ok"] === true) {
      this.captured = obj["result"] ?? null;
    } else {
      this.capturedError = obj["error"] as Record<string, unknown>;
    }
  }
  async recv(): Promise<null> {
    return null;
  }
  onMessage(): void {}
  offMessage(): void {}
  onClose(): void {}
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
   * Auto-detect parameter names from a handler function's source code.
   *
   * Works in both Node.js and browser since it uses `Function.toString()`.
   * Types are erased at runtime, so all detected params get type `"any"`.
   * This is a best-effort heuristic and may not be perfect in all cases, but it handles common patterns,
   * and is only used as a fallback when no explicit args were given at registration.
   */
  private static _detectHandlerParams(
    fn: AnyHandler,
  ): { name: string; optional: boolean; default?: string }[] {
    const src = fn.toString().trim();

    // Locate the parameter-list parentheses.
    let paramsStr = "";

    // Arrow with single param  e.g.  x => …  or  async x => …
    const singleArrow = /^(?:async\s+)?([$\w]+)\s*=>/.exec(src);
    if (singleArrow) {
      return [{ name: singleArrow[1], optional: false }];
    }

    // Parenthesised list  (…)
    const parenMatch = src.match(
      /^(?:async\s+)?(?:function\s*(?:\*\s*)?(?:\w+\s*)?)?\s*\(([^)]*)\)/,
    );
    if (parenMatch) {
      paramsStr = parenMatch[1];
    }

    if (!paramsStr.trim()) return [];

    // Split by top-level commas (ignoring those inside <…>, {…}, […], "…").
    const parts: string[] = [];
    let depth = 0;
    let current = "";
    for (const ch of paramsStr) {
      if (ch === "<" || ch === "{" || ch === "[" || ch === '"' || ch === "'") {
        depth++;
      } else if (
        ch === ">" ||
        ch === "}" ||
        ch === "]" ||
        ch === '"' ||
        ch === "'"
      ) {
        depth = Math.max(0, depth - 1);
      } else if (ch === "," && depth === 0) {
        parts.push(current);
        current = "";
        continue;
      }
      current += ch;
    }
    if (current.trim()) parts.push(current);

    // Parse each parameter.
    return parts
      .map((p) => {
        p = p.trim();
        if (!p) return null;

        // Destructuring, use a descriptive placeholder
        if (/^[{[]/.test(p)) {
          const placeholder = p
            .replace(/\s+/g, " ")
            .slice(0, 24)
            .replace(/[^$\w\s{}[\].,:?]/g, "");
          return { name: placeholder, optional: false };
        }

        // Rest parameter  ...name
        if (p.startsWith("...")) {
          return { name: p.slice(3).trim(), optional: false };
        }

        // Strip inline TypeScript type annotation  name: Type
        const nameOnly = p.split(":")[0].trim();

        // Default value  name = expr
        const eqIdx = nameOnly.indexOf("=");
        if (eqIdx !== -1) {
          const rawDefault = nameOnly.slice(eqIdx + 1).trim();
          return {
            name: nameOnly.slice(0, eqIdx).trim(),
            optional: true,
            default: rawDefault,
          };
        }

        return { name: nameOnly, optional: false };
      })
      .filter(Boolean) as {
      name: string;
      optional: boolean;
      default?: string;
    }[];
  }

  /**
   * Return the namespace schema as a plain {@link SaikuroSchema} object
   * suitable for schema announcement.
   *
   * When a handler was registered without explicit *args* or *returns*,
   * the missing information is auto-detected via `Function.toString()`
   * at schema-emission time.  This provides a useful schema in both
   * Node.js and browser environments without requiring a static analysis
   * pass over source files.
   */
  schemaObject(): SaikuroSchema {
    const functions: Record<string, WireFunctionSchema> = {};
    for (const [name, meta] of this._schema.entries()) {
      // If no explicit args were given, try auto-detection from the handler.
      const handler = this._handlers.get(name);
      const hasExplicitArgs = meta.args !== undefined;
      const hasExplicitReturns = meta.returns !== undefined;
      // Auto-detect params only when no explicit args were registered.
      const autoParams =
        !hasExplicitArgs && handler
          ? SaikuroProvider._detectHandlerParams(handler)
          : undefined;

      const wireArgs: WireFunctionSchema["args"] = hasExplicitArgs
        ? meta.args!.map((a) => {
            const wireArg: Record<string, unknown> = {
              name: a.name,
              type: a.type ?? { kind: "primitive", type: "any" },
            };
            if (a.optional !== undefined) wireArg["optional"] = a.optional;
            if (a.default !== undefined) wireArg["default"] = a.default;
            if (a.doc !== undefined) wireArg["doc"] = a.doc;
            return wireArg as unknown as import("./envelope").ArgumentDescriptor;
          })
        : autoParams
          ? autoParams.map((p) => ({
              name: p.name,
              type: { kind: "primitive", type: "any" } as const,
              optional: p.optional,
              ...(p.default !== undefined && { default: p.default }),
            }))
          : [];

      // When no explicit return type was given, infer from constructor name.
      let wireReturns: unknown;
      if (hasExplicitReturns) {
        wireReturns = meta.returns;
      } else if (handler) {
        const ctor = handler.constructor.name;
        if (ctor === "AsyncGeneratorFunction" || ctor === "GeneratorFunction") {
          wireReturns = {
            kind: "stream",
            item: { kind: "primitive", type: "any" },
          };
        } else {
          wireReturns = { kind: "primitive", type: "any" };
        }
      } else {
        wireReturns = { kind: "primitive", type: "any" };
      }

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
    return makeSchemaObject(this._namespace, functions);
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

    const fnName = envelope.target.includes(".")
      ? envelope.target.slice(envelope.target.lastIndexOf(".") + 1)
      : envelope.target;

    const handler = this._handlers.get(fnName);
    if (handler === undefined) {
      log.warn("dispatch: handler not found", {
        target: envelope.target,
        fnName,
      });
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

    log.debug("dispatch executing", {
      target: envelope.target,
      fnName,
      type: envelope.type,
      argsCount: envelope.args?.length ?? 0,
    });

    try {
      const result = handler(...(envelope.args as unknown[]));

      if (_isAsyncIterable(result)) {
        log.debug("dispatch streaming", { target: envelope.target });
        await this._dispatchStream(
          envelope,
          result as AsyncGenerator<unknown>,
          transport,
        );
        return;
      }

      const resolved = await Promise.resolve(result);
      if (!isCast) {
        await _sendOk(transport, envelope.id, resolved);
      }
    } catch (err) {
      log.error("dispatch error", {
        target: envelope.target,
        err: err instanceof Error ? err.message : String(err),
      });
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
        const sink = new _CaptureSink();

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

        if (sink.capturedError !== null) {
          const errCode =
            (sink.capturedError["code"] as string) ?? "ProviderError";
          const errMsg =
            (sink.capturedError["message"] as string) ?? "unknown error";
          await _sendError(transport, envelope.id, errCode, errMsg, {
            batch_index: i,
            target: item.target,
          });
          return;
        }

        results.push(sink.captured);
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
    log.info("serveOn starting", {
      namespace: this._namespace,
      dev: options?.dev ?? false,
      sourceFiles: options?.sourceFiles?.length
        ? options.sourceFiles.length
        : undefined,
      handlers: this._handlers.size,
    });

    if (options?.dev && Array.isArray(options.sourceFiles)) {
      try {
        const { extractSchema } = await import(
          "./schema_extractor"
        );
        const schema = await extractSchema(
          options.sourceFiles,
          this._namespace,
        );
        log.info("dev schema extracted, announcing", {
          namespace: this._namespace,
        });
        await this._announce(transport, schema as SaikuroSchema);
      } catch (err) {
        log.warn("dev schema extraction failed, falling back to built schema", {
          err: err instanceof Error ? err.message : String(err),
        });
        await this._announce(transport);
      }
    } else {
      await this._announce(transport);
    }

    log.info("serveOn entering dispatch loop", { namespace: this._namespace });
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
      const ack = await _waitForMessage(
        transport,
        () => transport.send(envelope),
        5000,
      );
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

/**
 * Register a one-shot message listener, invoke *sendFn*, and resolve with the
 * first message received, or `null` on timeout. Cleans up listener and timer
 * on all paths (success, timeout, send-failure).
 */
async function _waitForMessage(
  transport: Transport,
  sendFn: () => Promise<void>,
  timeoutMs: number,
): Promise<Record<string, unknown> | null> {
  return new Promise<Record<string, unknown> | null>((resolve, reject) => {
    const tid = setTimeout(() => {
      cleanup();
      resolve(null);
    }, timeoutMs);
    const onMsg = (raw: Record<string, unknown>): void => {
      cleanup();
      resolve(raw);
    };
    const cleanup = (): void => {
      clearTimeout(tid);
      transport.offMessage(onMsg);
    };
    transport.onMessage(onMsg);
    sendFn().catch((err: unknown) => {
      cleanup();
      reject(err);
    });
  });
}

function _isAsyncIterable(value: unknown): boolean {
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
      id: raw["id"] as Uint8Array,
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
    const isValidId =
      (envelope.id instanceof Uint8Array && envelope.id.length === 16) || false;
    if (!isValidId) {
      throw new TypeError(
        `missing or invalid 'id' field: expected 16-byte Uint8Array`,
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
  id: Uint8Array,
  result: unknown,
): Promise<void> {
  await transport.send({ id, ok: true, result });
}

async function _sendError(
  transport: Transport,
  id: Uint8Array,
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
