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

        JsonDocument doc;
        try
        {
            doc = JsonDocument.Parse(json);
        }
        catch (JsonException)
        {
            return JsonSerializer.Serialize(new SummaryResult("Invalid JSON input."), SummaryJsonContext.Default.SummaryResult);
        }

        using (doc)
        {
            var root = doc.RootElement;

            var preset = root.TryGetProperty("preset", out var presetEl) ? presetEl.GetString() ?? "balanced" : "balanced";

            int ascii = 0, bytes = 0;
            if (root.TryGetProperty("stats", out var stats))
            {
                ascii = stats.TryGetProperty("ascii", out var a) ? a.GetInt32() : 0;
                bytes = stats.TryGetProperty("bytes", out var b) ? b.GetInt32() : 0;
            }

            var label = "neutral";
            double score = 0;
            if (root.TryGetProperty("sentiment", out var sentiment))
            {
                label = sentiment.TryGetProperty("label", out var l) ? l.GetString() ?? "neutral" : "neutral";
                score = sentiment.TryGetProperty("score", out var s) ? s.GetDouble() : 0;
            }

            var topBigram = "";
            if (root.TryGetProperty("ngrams", out var ngrams)
                && ngrams.TryGetProperty("bigrams", out var bigrams)
                && bigrams.ValueKind == JsonValueKind.Array
                && bigrams.GetArrayLength() > 0
                && bigrams[0].ValueKind == JsonValueKind.Array
                && bigrams[0].GetArrayLength() > 0)
            {
                topBigram = bigrams[0][0].GetString() ?? "";
            }

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
}
