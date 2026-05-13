/**
 * Minimal structured logger for the Saikuro TypeScript adapter.
 *
 * Emits newline-delimited JSON records to `process.stderr` (Node.js) or
 * `globalThis.console.error` (browser / WASM).  Each record has the shape:
 *
 *   { ts, level, name, msg, ...extra }
 *
 * This keeps logs machine-parseable and capturable by any log aggregator,
 * unlike bare `console.error` calls which produce unstructured strings.
 *
 * Usage:
 *   const log = getLogger("saikuro.transport");
 *   log.debug("connected");
 *   log.error("frame decode failed", { err: String(e) });
 *
 * To forward logs to the Saikuro runtime:
 *   import { setLogSink, createTransportSink } from "@nisoku/saikuro";
 *   setLogSink(createTransportSink(client));
 */

export type LogLevel = "debug" | "info" | "warn" | "error";

export interface LogRecord {
  ts: string;
  level: LogLevel;
  name: string;
  msg: string;
  [key: string]: unknown;
}

// Sink

/**
 * The default sink writes JSON to stderr.  Replace this at startup to route
 * log records to a custom destination (e.g. a Saikuro log-envelope transport).
 */
let _sink: (record: LogRecord) => void = _defaultSink;

function _defaultSink(record: LogRecord): void {
  const line = JSON.stringify(record);
  // Node.js: process.stderr.write keeps output on stderr.
  // Browser/WASM: fallback to console.error (the only reliable channel).
  if (
    typeof process !== "undefined" &&
    typeof process.stderr?.write === "function"
  ) {
    process.stderr.write(line + "\n");
  } else {
    console.error(line);
  }
}

/** Replace the global log sink.  Affects all loggers. */
export function setLogSink(sink: (record: LogRecord) => void): void {
  _sink = sink;
}

/** Reset the log sink to the default (JSON to stderr). */
export function resetLogSink(): void {
  _sink = _defaultSink;
}

// Transport sink factory

/**
 * Create a log sink that forwards structured log records to the Saikuro
 * runtime as `log`-type envelopes.
 *
 * The send is fire-and-forget; errors are silently swallowed to prevent
 * infinite logging recursion.
 *
 * Usage:
 *   import { setLogSink, createTransportSink } from "@nisoku/saikuro";
 *   setLogSink(createTransportSink(client.transport));
 */
export interface TransportLike {
  send(obj: Record<string, unknown>): Promise<void>;
}

export function createTransportSink(
  transport: TransportLike,
): (record: LogRecord) => void {
  return (record: LogRecord): void => {
    const extraEntries = Object.entries(record).filter(
      ([k]) => k !== "ts" && k !== "level" && k !== "name" && k !== "msg",
    );
    const envelope: Record<string, unknown> = {
      version: 1,
      type: "log",
      id: `log-${record.ts}`,
      target: "$log",
      args: [
        {
          ts: record.ts,
          level: record.level,
          name: record.name,
          msg: record.msg,
          ...(extraEntries.length > 0
            ? { fields: Object.fromEntries(extraEntries) }
            : {}),
        },
      ],
    };

    // Fire-and-forget; ignore errors to prevent infinite recursion.
    transport.send(envelope).catch(() => {
      /* intentionally swallowed */
    });
  };
}

// Level filtering

const LEVEL_ORDER: Record<LogLevel, number> = {
  debug: 0,
  info: 1,
  warn: 2,
  error: 3,
};

let _minLevel: LogLevel = "info";

/** Set the minimum level at which records are emitted. */
export function setLogLevel(level: LogLevel): void {
  _minLevel = level;
}

// Logger

export class Logger {
  private readonly _name: string;

  constructor(name: string) {
    this._name = name;
  }

  private _emit(
    level: LogLevel,
    msg: string,
    extra?: Record<string, unknown>,
  ): void {
    if (LEVEL_ORDER[level] < LEVEL_ORDER[_minLevel]) return;
    const record: LogRecord = {
      ...extra,
      ts: new Date().toISOString(),
      level,
      name: this._name,
      msg,
    };
    _sink(record);
  }

  debug(msg: string, extra?: Record<string, unknown>): void {
    this._emit("debug", msg, extra);
  }

  info(msg: string, extra?: Record<string, unknown>): void {
    this._emit("info", msg, extra);
  }

  warn(msg: string, extra?: Record<string, unknown>): void {
    this._emit("warn", msg, extra);
  }

  error(msg: string, extra?: Record<string, unknown>): void {
    this._emit("error", msg, extra);
  }
}

/** Create (or return a cached) named logger. */
const _loggers = new Map<string, Logger>();

export function getLogger(name: string): Logger {
  let logger = _loggers.get(name);
  if (logger === undefined) {
    logger = new Logger(name);
    _loggers.set(name, logger);
  }
  return logger;
}
