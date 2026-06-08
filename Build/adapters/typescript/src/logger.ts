import { createSatori } from "@nisoku/satori";
import type { SatoriInstance } from "@nisoku/satori";

// Global Satori instance
const _satori: SatoriInstance = createSatori({
  logLevel: "debug",
  enableConsole: true,
  enableCallsite: false,
  enableEnvInfo: false,
  enableStateSnapshot: false,
  enableCausalLinks: false,
  enableMetrics: false,
  maxBufferSize: 2000,
});

// Type exports
export type LogLevel = "debug" | "info" | "warn" | "error";

export interface LogRecord {
  ts: string;
  level: LogLevel;
  name: string;
  msg: string;
  [key: string]: unknown;
}

// Level filtering
const LEVEL_ORDER: Record<LogLevel, number> = {
  debug: 0,
  info: 1,
  warn: 2,
  error: 3,
};

let _minLevel: LogLevel = "debug";

export function setLogLevel(level: LogLevel): void {
  _minLevel = level;
}

// Sink support
type SinkFn = (record: LogRecord) => void;

let _sinkUnsub: (() => void) | null = null;

export function setLogSink(sink: SinkFn): void {
  if (_sinkUnsub) _sinkUnsub();
  _sinkUnsub = _satori.bus.subscribe((entry) => {
    const record: LogRecord = {
      ts: new Date(entry.timestamp).toISOString(),
      level: (entry.level === "debug" ||
      entry.level === "info" ||
      entry.level === "warn" ||
      entry.level === "error"
        ? entry.level
        : "info") as LogLevel,
      name: entry.scope,
      msg: entry.message,
      ...(entry.tags.length > 0 ? { tags: entry.tags } : {}),
      ...(entry.state ? { state: entry.state } : {}),
    };
    sink(record);
  });
}

export function resetLogSink(): void {
  if (_sinkUnsub) {
    _sinkUnsub();
    _sinkUnsub = null;
  }
}

// Transport sink factory
export interface TransportLike {
  send(obj: object): Promise<void>;
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
    transport.send(envelope).catch(() => {});
  };
}

// Logger
export class Logger {
  private readonly _satoriLogger: ReturnType<SatoriInstance["createLogger"]>;

  constructor(_name: string) {
    this._satoriLogger = _satori.createLogger(_name);
  }

  private _shouldEmit(level: LogLevel): boolean {
    return LEVEL_ORDER[level] >= LEVEL_ORDER[_minLevel];
  }

  debug(msg: string, extra?: Record<string, unknown>): void {
    if (!this._shouldEmit("debug")) return;
    this._satoriLogger.debug(msg, extra ? { state: extra } : undefined);
  }

  info(msg: string, extra?: Record<string, unknown>): void {
    if (!this._shouldEmit("info")) return;
    this._satoriLogger.info(msg, extra ? { state: extra } : undefined);
  }

  warn(msg: string, extra?: Record<string, unknown>): void {
    if (!this._shouldEmit("warn")) return;
    this._satoriLogger.warn(msg, extra ? { state: extra } : undefined);
  }

  error(msg: string, extra?: Record<string, unknown>): void {
    if (!this._shouldEmit("error")) return;
    this._satoriLogger.error(msg, extra ? { state: extra } : undefined);
  }
}

// Logger cache
const _loggers = new Map<string, Logger>();

export function getLogger(name: string): Logger {
  let logger = _loggers.get(name);
  if (logger === undefined) {
    logger = new Logger(name);
    _loggers.set(name, logger);
  }
  return logger;
}
