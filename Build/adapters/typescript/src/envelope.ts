/**
 * Saikuro wire protocol types.
 *
 * These mirror the Rust `saikuro_core::envelope` module exactly.
 * We use plain TypeScript interfaces so they can be constructed and
 * pattern-matched without instanceof checks.
 */

// Protocol constant

export const PROTOCOL_VERSION = 1 as const;

// Invocation types

export type InvocationType =
  | "call"
  | "cast"
  | "stream"
  | "channel"
  | "batch"
  | "resource"
  | "log"
  | "announce";

export type StreamControl = "end" | "pause" | "resume" | "abort";

// Envelopes

/** Outbound invocation envelope. */
export interface Envelope {
  readonly version: number;
  readonly type: InvocationType;
  readonly id: string;
  readonly target: string;
  readonly args: readonly unknown[];
  readonly meta?: Readonly<Record<string, unknown>>;
  readonly capability?: string;
  readonly batch_items?: readonly Envelope[];
  readonly stream_control?: StreamControl;
  readonly seq?: number;
}

/** Inbound response envelope. */
export interface ResponseEnvelope {
  readonly id: string;
  readonly ok: boolean;
  readonly result?: unknown;
  readonly error?: ErrorPayload;
  readonly seq?: number;
  readonly stream_control?: StreamControl;
}

// Error payload

export interface ErrorPayload {
  readonly code: ErrorCode;
  readonly message: string;
  readonly details?: Readonly<Record<string, unknown>>;
}

export type ErrorCode =
  | "NamespaceNotFound"
  | "FunctionNotFound"
  | "InvalidArguments"
  | "IncompatibleVersion"
  | "MalformedEnvelope"
  | "NoProvider"
  | "ProviderUnavailable"
  | "BatchRoutingConflict"
  | "CapabilityDenied"
  | "CapabilityInvalid"
  | "ConnectionLost"
  | "MessageTooLarge"
  | "Timeout"
  | "BufferOverflow"
  | "ProviderError"
  | "ProviderPanic"
  | "StreamClosed"
  | "ChannelClosed"
  | "OutOfOrder"
  | "Internal";

// Factory helpers

export function makeCallEnvelope(
  target: string,
  args: readonly unknown[],
  capability?: string,
): Envelope {
  return {
    version: PROTOCOL_VERSION,
    type: "call",
    id: generateId(),
    target,
    args,
    ...(capability !== undefined && { capability }),
  };
}

export function makeCastEnvelope(
  target: string,
  args: readonly unknown[],
  capability?: string,
): Envelope {
  return { ...makeCallEnvelope(target, args, capability), type: "cast" };
}

export function makeStreamOpenEnvelope(
  target: string,
  args: readonly unknown[],
): Envelope {
  return { ...makeCallEnvelope(target, args), type: "stream" };
}

export function makeChannelOpenEnvelope(
  target: string,
  args: readonly unknown[],
): Envelope {
  return { ...makeCallEnvelope(target, args), type: "channel" };
}

/** Schema type used in announce envelopes. */
export interface SaikuroSchema {
  readonly version: number;
  readonly namespaces: Readonly<Record<string, NamespaceSchema>>;
  readonly types?: Readonly<Record<string, unknown>>;
}

export interface NamespaceSchema {
  readonly functions: Readonly<Record<string, FunctionSchema>>;
  readonly doc?: string;
}

export interface FunctionSchema {
  readonly args?: readonly ArgumentDescriptor[];
  readonly returns?: unknown;
  readonly visibility?: "public" | "internal" | "private";
  readonly capabilities?: readonly string[];
  readonly idempotent?: boolean;
  readonly doc?: string;
}

export interface ArgumentDescriptor {
  readonly name: string;
  readonly type: unknown;
  readonly optional?: boolean;
  readonly default?: unknown;
  readonly doc?: string;
}

// Resource handle

/**
 * An opaque reference to large or external data.
 *
 * Returned as the `result` of a `resource`-type invocation.  The handle
 * carries enough metadata for the recipient to identify, size, type, and
 * optionally open the data without transferring it inline.
 *
 * Wire format (flat MessagePack map):
 * ```json
 * { "id": "<uuid>", "mime_type": "image/png", "size": 4096, "uri": "saikuro://res/<id>" }
 * ```
 * All fields except `id` are optional.
 */
export interface ResourceHandle {
  readonly id: string;
  readonly mime_type?: string;
  readonly size?: number;
  readonly uri?: string;
}

/**
 * Decode a raw result value (as returned by the runtime) into a
 * {@link ResourceHandle}.
 *
 * Returns `null` when `raw` is `null`, `undefined`, or does not look like a
 * resource handle map (i.e. is missing the required `id` field).
 */
export function decodeResourceHandle(raw: unknown): ResourceHandle | null {
  if (raw === null || raw === undefined || typeof raw !== "object") {
    return null;
  }
  const map = raw as Record<string, unknown>;
  if (typeof map["id"] !== "string") {
    return null;
  }
  const scratch: Record<string, unknown> = { id: map["id"] as string };
  if (typeof map["mime_type"] === "string")
    scratch["mime_type"] = map["mime_type"];
  if (typeof map["size"] === "number") scratch["size"] = map["size"];
  if (typeof map["uri"] === "string") scratch["uri"] = map["uri"];
  return scratch as unknown as ResourceHandle;
}

/**
 * Construct a resource-access envelope.
 *
 * `target` identifies the provider function that manages the resource
 * (e.g. `"files.open"`).  `args` are provider-specific parameters that
 * identify or parameterise the resource (e.g. a resource ID or byte range).
 * The provider returns a {@link ResourceHandle} in the response `result`.
 */
export function makeResourceEnvelope(
  target: string,
  args: readonly unknown[],
  capability?: string,
): Envelope {
  return { ...makeCallEnvelope(target, args, capability), type: "resource" };
}

/**
 * Construct a batch invocation envelope.
 *
 * `items` are individual call envelopes (built with {@link makeCallEnvelope}).
 * The runtime dispatches each in order and returns a single response whose
 * `result` is an array of per-call results, in the same order.
 */
export function makeBatchEnvelope(items: readonly Envelope[]): Envelope {
  return {
    version: PROTOCOL_VERSION,
    type: "batch",
    id: generateId(),
    target: "",
    args: [],
    batch_items: items,
  };
}

/**
 * Construct a schema-announcement envelope.
 *
 * `schema` is a plain-object representation of the Saikuro Schema that will be
 * embedded in `args[0]`.  The runtime deserialises it and merges it into the
 * live registry.
 */
export function makeAnnounceEnvelope(schema: SaikuroSchema): Envelope {
  return {
    version: PROTOCOL_VERSION,
    type: "announce",
    id: generateId(),
    target: "$saikuro.announce",
    args: [schema],
  };
}

// UUID v4

function generateId(): string {
  // Prefer the Web Crypto API (available in browsers, Node.js ≥ 19, and WASM
  // environments) for cryptographically-random UUIDs.
  if (
    typeof globalThis.crypto !== "undefined" &&
    typeof globalThis.crypto.randomUUID === "function"
  ) {
    return globalThis.crypto.randomUUID();
  }
  // Fallback for older Node.js: simple random UUID v4.
  return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === "x" ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}

export { generateId };
