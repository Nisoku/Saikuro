/**
 * SaikuroLoggingHandler:  a TypeScript wrapper that forwards structured log
 * records to the Saikuro runtime over a transport connection.
 *
 * Instead of writing to stderr (which is lost when the process is not attached to
 * a terminal), this handler serialises each log record as a Saikuro
 * `log` envelope and sends it over the provided transport. The runtime's router
 * then intercepts the envelope and passes it to the configured log sink (e.g.
 * structured JSON to stdout, or into the runtime's own tracing subscriber).
 *
 * Usage:
 *
 *   import { SaikuroClient } from "@nisoku/saikuro";
 *   import { createLoggingHandler } from "@nisoku/saikuro/logging_handler";
 *
 *   const client = await SaikuroClient.connect("unix:///tmp/saikuro.sock");
 *   const handler = createLoggingHandler(client);
 *
 *   // Use with a logger (e.g., pino, console)
 *   console.log = handler;
 *
 *   // Or use directly:
 *   handler("info", "my-logger", "Hello world", { extra: "field" });
 */

import type { Transport } from "./transport";
import { PROTOCOL_VERSION } from "./envelope";

export type LogLevel = "trace" | "debug" | "info" | "warn" | "error";

interface LoggerMethod {
  (message: string, ...args: unknown[]): unknown;
}

interface Logger {
  name?: string;
  trace?: LoggerMethod;
  debug?: LoggerMethod;
  info?: LoggerMethod;
  warn?: LoggerMethod;
  error?: LoggerMethod;
  log?: (level: string, message: string, ...args: unknown[]) => unknown;
}

/**
 * Convert a log level string to Saikuro LogLevel.
 */
function toSaikuroLevel(level: string): LogLevel {
  const l = level.toLowerCase();
  if (l === "trace" || l === "verbose") return "trace";
  if (l === "debug") return "debug";
  if (l === "info" || l === "log") return "info";
  if (l === "warn" || l === "warning") return "warn";
  return "error";
}

/**
 * Create a logging handler function that forwards logs to the Saikuro runtime.
 *
 * @param transport - An open transport connected to the Saikuro runtime.
 * @returns A function that can be called with (level, name, message, fields).
 */
export function createLoggingHandler(transport: Transport) {
  return function log(
    level: string,
    name: string,
    msg: string,
    fields?: Record<string, unknown>
  ): void {
    const ts = new Date().toISOString();
    const saikuroLevel = toSaikuroLevel(level);

    const logRecord: Record<string, unknown> = {
      ts,
      level: saikuroLevel,
      name,
      msg,
    };

    if (fields && Object.keys(fields).length > 0) {
      logRecord.fields = fields;
    }

    const envelope: Record<string, unknown> = {
      version: PROTOCOL_VERSION,
      type: "log",
      id: `log-${ts}`,
      target: "$log",
      args: [logRecord],
    };

    // Fire-and-forget; swallow errors to prevent infinite recursion.
    transport
      .send(envelope)
      .catch(() => {});
  };
}

/**
 * Wrap a logger object to forward all its log calls to Saikuro.
 *
 * @param transport - An open transport connected to the Saikuro runtime.
 * @param logger - A logger with methods like log(), info(), warn(), error(), debug(), trace().
 * @returns A wrapped logger that forwards to Saikuro.
 */
export function wrapLogger(transport: Transport, logger: Logger): Logger {
  const handler = createLoggingHandler(transport);

  const wrapped: Logger = { ...logger };

  const levels: LogLevel[] = ["trace", "debug", "info", "warn", "error"];

  for (const level of levels) {
    const orig = logger[level];
    if (typeof orig === "function") {
      wrapped[level] = (msg: string, ...args: unknown[]) => {
        const fields = args.length > 0 ? { args } : undefined;
        handler(level, logger.name ?? "logger", msg, fields);
        return orig.call(logger, msg, ...args);
      };
    }
  }

  // Handle custom log method
  if (typeof logger.log === "function") {
    const origLog = logger.log;
    wrapped.log = (level: string, msg: string, ...args: unknown[]) => {
      const fields = args.length > 0 ? { args } : undefined;
      handler(level, logger.name ?? "logger", msg, fields);
      return origLog.call(logger, level, msg, ...args);
    };
  }

  return wrapped;
}
