using System.Diagnostics;
using System.Runtime.CompilerServices;
using System.Text.Json;

namespace Saikuro.Tests;

public class SchemaExtractorTests
{
    [Fact]
    public void ExtractorTool_PrintsValidSchemaJson()
    {
        var repoRoot = FindRepoRoot();
        var extractorProject = Path.Combine(repoRoot, "Build", "adapters", "csharp", "tools", "extractor", "extractor.csproj");

        var result = RunProcess(
            "dotnet",
            $"run --project \"{extractorProject}\" parityns"
        );

        Assert.Equal(0, result.exitCode);

        var jsonText = ExtractJson(result.stdout);
        using var doc = JsonDocument.Parse(jsonText);

        var root = doc.RootElement;
        Assert.True(root.TryGetProperty("version", out _));
        Assert.True(root.TryGetProperty("namespaces", out var ns));
        Assert.True(ns.TryGetProperty("parityns", out var parity));
        Assert.True(parity.TryGetProperty("functions", out var functions));
        Assert.True(functions.TryGetProperty("Add", out _));
    }

    [Fact]
    public void ExtractorTool_SupportsCustomNamespaceArgument()
    {
        var repoRoot = FindRepoRoot();
        var extractorProject = Path.Combine(repoRoot, "Build", "adapters", "csharp", "tools", "extractor", "extractor.csproj");

        var result = RunProcess(
            "dotnet",
            $"run --project \"{extractorProject}\" custom_ns"
        );

        Assert.Equal(0, result.exitCode);

        var jsonText = ExtractJson(result.stdout);
        using var doc = JsonDocument.Parse(jsonText);

        var ns = doc.RootElement.GetProperty("namespaces");
        Assert.True(ns.TryGetProperty("custom_ns", out _));
    }

    private static (int exitCode, string stdout, string stderr) RunProcess(string fileName, string arguments)
    {
        var psi = new ProcessStartInfo
        {
            FileName = fileName,
            Arguments = arguments,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            CreateNoWindow = true,
        };

        using var proc = Process.Start(psi)!;
        var stdoutTask = proc.StandardOutput.ReadToEndAsync();
        var stderrTask = proc.StandardError.ReadToEndAsync();
        proc.WaitForExit();
        Task.WaitAll(stdoutTask, stderrTask);

        return (proc.ExitCode, stdoutTask.Result, stderrTask.Result);
    }

    private static string ExtractJson(string text)
    {
        var start = text.IndexOf('{');
        var end = text.LastIndexOf('}');
        Assert.True(start >= 0 && end >= 0 && end > start, $"No JSON found in output: {text}");
        return text.Substring(start, end - start + 1);
    }

    private static string FindRepoRoot([CallerFilePath] string sourceFilePath = "")
    {
        var startDirs = new List<string>();

        if (!string.IsNullOrWhiteSpace(sourceFilePath))
        {
            var sourceDir = Path.GetDirectoryName(sourceFilePath);
            if (!string.IsNullOrWhiteSpace(sourceDir))
            {
                startDirs.Add(sourceDir);
            }
        }

        startDirs.Add(AppContext.BaseDirectory);
        startDirs.Add(Directory.GetCurrentDirectory());

        foreach (var start in startDirs.Distinct(StringComparer.OrdinalIgnoreCase))
        {
            var current = new DirectoryInfo(start);
            while (current is not null)
            {
                if (File.Exists(Path.Combine(current.FullName, "Saikuro.sln")))
                {
                    return current.FullName;
                }
                current = current.Parent;
            }
        }

        throw new InvalidOperationException("Could not locate repository root (searched from source path, base directory, and current directory).");
    }
}
