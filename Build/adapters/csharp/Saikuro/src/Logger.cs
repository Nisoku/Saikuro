// Saikuro structured logger.
//
// Emits newline-delimited JSON records to stderr.
// A global sink can be replaced (e.g. to forward to the Saikuro runtime).
//
// Usage:
//   var log = SaikuroLogger.GetLogger("saikuro.transport");
//   log.Debug("connected");
//   log.Error("frame decode failed", someException.Message);
//
// To forward logs to the runtime:
//   SaikuroLogger.SetSink(SaikuroLogger.CreateTransportSink(transport));

using System.Text.Json;

namespace Saikuro;

// LogLevel

public enum LogLevel
{
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

internal static class LogLevelExt
{
    internal static string ToWire(this LogLevel level) =>
        level switch
        {
            LogLevel.Trace => "trace",
            LogLevel.Debug => "debug",
            LogLevel.Info => "info",
            LogLevel.Warn => "warn",
            LogLevel.Error => "error",
            _ => "info",
        };
}

// Log record

public sealed class LogRecord
{
    public string Ts { get; init; } = DateTimeOffset.UtcNow.ToString("O");
    public string Level { get; init; } = "info";
    public string Name { get; init; } = "";
    public string Msg { get; init; } = "";
    public IReadOnlyDictionary<string, object?>? Fields { get; init; }
}

// Sink

public static class SaikuroLogSink
{
    private static Action<LogRecord> _sink = DefaultSink;

    private static void DefaultSink(LogRecord record)
    {
        var obj = new Dictionary<string, object?>
        {
            ["ts"] = record.Ts,
            ["level"] = record.Level,
            ["name"] = record.Name,
            ["msg"] = record.Msg,
        };
        if (record.Fields is { Count: > 0 })
            obj["fields"] = record.Fields;
        var line = JsonSerializer.Serialize(obj);
        Console.Error.WriteLine(line);
    }

    /// <summary>Replace the global log sink.  Affects all loggers.</summary>
    public static void SetSink(Action<LogRecord> sink) => _sink = sink;

    /// <summary>Reset the log sink to the default (JSON to stderr).</summary>
    public static void ResetSink() => _sink = DefaultSink;

    internal static void Emit(LogRecord record) => _sink(record);

    /// <summary>
    /// Create a sink that forwards log records to the Saikuro runtime
    /// as <c>log</c>-type envelopes (fire-and-forget).
    /// </summary>
    public static Action<LogRecord> CreateTransportSink(ITransport transport) =>
        record =>
        {
            var logObj = new Dictionary<string, object?>
            {
                ["ts"] = record.Ts,
                ["level"] = record.Level,
                ["name"] = record.Name,
                ["msg"] = record.Msg,
            };
            if (record.Fields is { Count: > 0 })
                logObj["fields"] = record.Fields;

            var envelope = new Dictionary<string, object?>
            {
                ["version"] = (int)Protocol.Version,
                ["type"] = "log",
                [WireKey.Id] = $"{WireKey.LogIdPrefix}{record.Ts}",
                [WireKey.Target] = WireKey.LogTarget,
                ["args"] = new object?[] { logObj },
            };
            // Fire-and-forget; swallow errors to prevent infinite recursion.
            _ = transport.SendAsync(envelope).ContinueWith(_ => { }, TaskScheduler.Default);
        };
}

// Logger

/// <summary>Named structured logger.</summary>
public sealed class SaikuroLogger
{
    private static LogLevel _minLevel = LogLevel.Info;

    private readonly string _name;

    private SaikuroLogger(string name) => _name = name;

    // Level

    /// <summary>Set the minimum level at which records are emitted.</summary>
    public static void SetLogLevel(LogLevel level) => _minLevel = level;

    // Cache

    private static readonly Dictionary<string, SaikuroLogger> Loggers = new();

    /// <summary>Return a (cached) named logger.</summary>
    public static SaikuroLogger GetLogger(string name)
    {
        lock (Loggers)
        {
            if (!Loggers.TryGetValue(name, out var logger))
            {
                logger = new SaikuroLogger(name);
                Loggers[name] = logger;
            }
            return logger;
        }
    }

    // Emit

    private void Emit(
        LogLevel level,
        string msg,
        IReadOnlyDictionary<string, object?>? fields = null
    )
    {
        if (level < _minLevel)
            return;
        SaikuroLogSink.Emit(
            new LogRecord
            {
                Ts = DateTimeOffset.UtcNow.ToString("O"),
                Level = level.ToWire(),
                Name = _name,
                Msg = msg,
                Fields = fields,
            }
        );
    }

    public void Trace(string msg, IReadOnlyDictionary<string, object?>? fields = null) =>
        Emit(LogLevel.Trace, msg, fields);

    public void Debug(string msg, IReadOnlyDictionary<string, object?>? fields = null) =>
        Emit(LogLevel.Debug, msg, fields);

    public void Info(string msg, IReadOnlyDictionary<string, object?>? fields = null) =>
        Emit(LogLevel.Info, msg, fields);

    public void Warn(string msg, IReadOnlyDictionary<string, object?>? fields = null) =>
        Emit(LogLevel.Warn, msg, fields);

    public void Error(string msg, IReadOnlyDictionary<string, object?>? fields = null) =>
        Emit(LogLevel.Error, msg, fields);

    // Convenience overload that takes a plain string detail.
    public void Error(string msg, string detail) =>
        Emit(LogLevel.Error, msg, new Dictionary<string, object?> { ["detail"] = detail });

    public void Warn(string msg, string detail) =>
        Emit(LogLevel.Warn, msg, new Dictionary<string, object?> { ["detail"] = detail });
}
