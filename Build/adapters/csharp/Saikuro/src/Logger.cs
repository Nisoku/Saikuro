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

using System.Collections.Concurrent;
using System.Text.Json;
using System.Threading.Channels;
using Microsoft.Extensions.Logging;

namespace Saikuro;

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

    internal static Microsoft.Extensions.Logging.LogLevel ToMel(this LogLevel level) =>
        level switch
        {
            LogLevel.Trace => Microsoft.Extensions.Logging.LogLevel.Trace,
            LogLevel.Debug => Microsoft.Extensions.Logging.LogLevel.Debug,
            LogLevel.Info => Microsoft.Extensions.Logging.LogLevel.Information,
            LogLevel.Warn => Microsoft.Extensions.Logging.LogLevel.Warning,
            LogLevel.Error => Microsoft.Extensions.Logging.LogLevel.Error,
            _ => Microsoft.Extensions.Logging.LogLevel.Information,
        };

    internal static Saikuro.LogLevel FromMel(Microsoft.Extensions.Logging.LogLevel level) =>
        level switch
        {
            Microsoft.Extensions.Logging.LogLevel.Trace => Saikuro.LogLevel.Trace,
            Microsoft.Extensions.Logging.LogLevel.Debug => Saikuro.LogLevel.Debug,
            Microsoft.Extensions.Logging.LogLevel.Information => Saikuro.LogLevel.Info,
            Microsoft.Extensions.Logging.LogLevel.Warning => Saikuro.LogLevel.Warn,
            Microsoft.Extensions.Logging.LogLevel.Error => Saikuro.LogLevel.Error,
            _ => Saikuro.LogLevel.Info,
        };
}

public sealed class LogRecord
{
    public string Ts { get; init; } = DateTimeOffset.UtcNow.ToString("O");
    public string Level { get; init; } = "info";
    public string Name { get; init; } = "";
    public string Msg { get; init; } = "";
    public IReadOnlyDictionary<string, object?>? Fields { get; init; }
}

public static class SaikuroLogSink
{
    private static readonly object _lock = new();
    private static ILoggerFactory? _factory;
    private static Action<LogRecord> _sink = DefaultSink;
    private static CancellationTokenSource? _activeTransportCts;
    private static Channel<LogRecord>? _activeTransportChannel;

    private static void DefaultSink(LogRecord record)
    {
        var obj = new Dictionary<string, object?>
        {
            [WireKey.Ts] = record.Ts,
            [WireKey.Level] = record.Level,
            [WireKey.Name] = record.Name,
            [WireKey.Msg] = record.Msg,
        };
        if (record.Fields is { Count: > 0 })
            obj[WireKey.Fields] = record.Fields;
        var line = JsonSerializer.Serialize(obj);
        Console.Error.WriteLine(line);
    }

    public static void SetSink(Action<LogRecord> sink)
    {
        StopTransportSink();
        lock (_lock) { _sink = sink; }
    }

    public static void ResetSink()
    {
        StopTransportSink();
        lock (_lock) { _sink = DefaultSink; }
    }

    private static void StopTransportSink()
    {
        CancellationTokenSource? cts;
        Channel<LogRecord>? ch;
        lock (_lock)
        {
            cts = _activeTransportCts;
            ch = _activeTransportChannel;
            _activeTransportCts = null;
            _activeTransportChannel = null;
        }
        ch?.Writer.TryComplete();
        cts?.Cancel();
        cts?.Dispose();
    }

    public static void SetLoggerFactory(ILoggerFactory factory)
    {
        lock (_lock) { _factory = factory; }
    }

    internal static void Emit(LogRecord record)
    {
        Action<LogRecord> sink;
        lock (_lock) { sink = _sink; }
        sink(record);
    }

    internal static ILogger CreateLogger(string name)
    {
        lock (_lock)
        {
            return _factory is not null
                ? _factory.CreateLogger(name)
                : new SaikuroInternalLogger(name);
        }
    }

    public static Action<LogRecord> CreateTransportSink(ITransport transport)
    {
        StopTransportSink();

        var channel = Channel.CreateUnbounded<LogRecord>(new UnboundedChannelOptions
        {
            SingleReader = true,
            SingleWriter = false,
        });
        var cts = new CancellationTokenSource();
        var token = cts.Token;

        lock (_lock)
        {
            _activeTransportCts = cts;
            _activeTransportChannel = channel;
        }

        _ = Task.Run(async () =>
        {
            try
            {
                await foreach (var record in channel.Reader.ReadAllAsync(token))
                {
                    try
                    {
                        var logObj = new Dictionary<string, object?>
                        {
                            [WireKey.Ts] = record.Ts,
                            [WireKey.Level] = record.Level,
                            [WireKey.Name] = record.Name,
                            [WireKey.Msg] = record.Msg,
                        };
                        if (record.Fields is { Count: > 0 })
                            logObj[WireKey.Fields] = record.Fields;

                        var envelope = new Dictionary<string, object?>
                        {
                            [WireKey.Version] = (int)Protocol.Version,
                            [WireKey.Type] = "log",
                            [WireKey.Id] = $"{WireKey.LogIdPrefix}{record.Ts}",
                            [WireKey.Target] = WireKey.LogTarget,
                            [WireKey.Args] = new object?[] { logObj },
                        };
                        await transport.SendAsync(envelope).ConfigureAwait(false);
                    }
                    catch
                    {
                        // swallow to prevent infinite recursion
                    }
                }
            }
            catch (OperationCanceledException)
            {
                // shutdown requested, exit cleanly
            }
        }, token);

        return record => channel.Writer.TryWrite(record);
    }
}

internal sealed class LogEntry
{
    public string Msg { get; }
    public IReadOnlyDictionary<string, object?>? Fields { get; }
    public LogEntry(string msg, IReadOnlyDictionary<string, object?>? fields)
    {
        Msg = msg;
        Fields = fields;
    }
}

internal sealed class SaikuroInternalLogger : ILogger
{
    private readonly string _name;

    internal SaikuroInternalLogger(string name) => _name = name;

    public IDisposable? BeginScope<TState>(TState state) where TState : notnull => null;

    public bool IsEnabled(Microsoft.Extensions.Logging.LogLevel logLevel) => true;

    public void Log<TState>(
        Microsoft.Extensions.Logging.LogLevel logLevel,
        EventId eventId,
        TState state,
        Exception? exception,
        Func<TState, Exception?, string> formatter)
    {
        IReadOnlyDictionary<string, object?>? fields = null;
        if (state is LogEntry entry)
            fields = entry.Fields;
        var record = new LogRecord
        {
            Ts = DateTimeOffset.UtcNow.ToString("O"),
            Level = LogLevelExt.FromMel(logLevel).ToWire(),
            Name = _name,
            Msg = formatter(state, exception),
            Fields = fields,
        };
        SaikuroLogSink.Emit(record);
    }
}

public sealed class SaikuroLogger
{
    private static volatile Microsoft.Extensions.Logging.LogLevel _minLevel = Microsoft.Extensions.Logging.LogLevel.Information;
    private static readonly ConcurrentDictionary<string, SaikuroLogger> Loggers = new();

    private readonly ILogger _logger;

    private SaikuroLogger(string name)
    {
        _logger = SaikuroLogSink.CreateLogger(name);
    }

    public static void SetLogLevel(Saikuro.LogLevel level) => _minLevel = level.ToMel();

    public static SaikuroLogger GetLogger(string name) =>
        Loggers.GetOrAdd(name, static n => new SaikuroLogger(n));

    private bool IsEnabled(Microsoft.Extensions.Logging.LogLevel level) => level >= _minLevel;

    private void Log(Microsoft.Extensions.Logging.LogLevel level, string msg, IReadOnlyDictionary<string, object?>? fields = null)
    {
        if (!IsEnabled(level))
            return;
        _logger.Log(level, 0, new LogEntry(msg, fields), null, static (m, _) => m.Msg);
    }

    public void Trace(string msg, IReadOnlyDictionary<string, object?>? fields = null) =>
        Log(Microsoft.Extensions.Logging.LogLevel.Trace, msg, fields);

    public void Debug(string msg, IReadOnlyDictionary<string, object?>? fields = null) =>
        Log(Microsoft.Extensions.Logging.LogLevel.Debug, msg, fields);

    public void Info(string msg, IReadOnlyDictionary<string, object?>? fields = null) =>
        Log(Microsoft.Extensions.Logging.LogLevel.Information, msg, fields);

    public void Warn(string msg, IReadOnlyDictionary<string, object?>? fields = null) =>
        Log(Microsoft.Extensions.Logging.LogLevel.Warning, msg, fields);

    public void Error(string msg, IReadOnlyDictionary<string, object?>? fields = null) =>
        Log(Microsoft.Extensions.Logging.LogLevel.Error, msg, fields);

    public void ErrorWithDetail(string msg, string detail) =>
        Log(Microsoft.Extensions.Logging.LogLevel.Error, msg,
            new Dictionary<string, object?> { ["detail"] = detail });

    public void WarnWithDetail(string msg, string detail) =>
        Log(Microsoft.Extensions.Logging.LogLevel.Warning, msg,
            new Dictionary<string, object?> { ["detail"] = detail });
}
