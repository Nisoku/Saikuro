using System;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Runtime.InteropServices.JavaScript;

namespace InsightLab;

public record SummaryResult([property: JsonPropertyName("text")] string Text);

[JsonSerializable(typeof(SummaryResult))]
public partial class SummaryJsonContext : JsonSerializerContext { }

public static partial class SummaryEngine
{
    [JSExport]
    public static string Summarize(string json)
    {
        if (string.IsNullOrWhiteSpace(json))
        {
            return JsonSerializer.Serialize(new SummaryResult("No input provided."), SummaryJsonContext.Default.SummaryResult);
        }

        using var doc = JsonDocument.Parse(json);
        var root = doc.RootElement;

        var preset = root.GetProperty("preset").GetString() ?? "balanced";
        var stats = root.GetProperty("stats");
        var sentiment = root.GetProperty("sentiment");
        var ngrams = root.GetProperty("ngrams");

        var ascii = stats.GetProperty("ascii").GetInt32();
        var bytes = stats.GetProperty("bytes").GetInt32();
        var label = sentiment.GetProperty("label").GetString() ?? "neutral";
        var score = sentiment.GetProperty("score").GetDouble();
        var topBigram = ngrams.GetProperty("bigrams")[0][0].GetString() ?? "";

        var ratio = bytes == 0 ? 0.0 : (double)ascii / bytes;
        var message = preset switch
        {
            "precision" => $"Precision mode: {label} tone with score {score:F2}. Top phrase '{topBigram}'.",
            "story" => $"Narrative mode: {label} tone and a crisp lead phrase '{topBigram}'.",
            _ => $"Balanced mode: {label} tone. ASCII ratio {ratio:F2}. Top phrase '{topBigram}'."
        };

        return JsonSerializer.Serialize(new SummaryResult(message), SummaryJsonContext.Default.SummaryResult);
    }
}
