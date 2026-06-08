using System;
using System.Collections.Generic;
using System.Text.Json;

namespace InsightLab;

public static class SummaryEngine
{
    public static Dictionary<string, object?> ComputeSummary(string json)
    {
        if (string.IsNullOrWhiteSpace(json))
        {
            return new Dictionary<string, object?> { ["text"] = "No input provided." };
        }

        JsonDocument doc;
        try
        {
            doc = JsonDocument.Parse(json);
        }
        catch (JsonException)
        {
            return new Dictionary<string, object?> { ["text"] = "Invalid JSON input." };
        }

        using (doc)
        {
            var root = doc.RootElement;

            var preset = root.TryGetProperty("preset", out var presetEl)
                ? presetEl.GetString() ?? "balanced"
                : "balanced";

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
                label = sentiment.TryGetProperty("label", out var l)
                    ? l.GetString() ?? "neutral"
                    : "neutral";
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
                "precision" =>
                    $"Precision mode: {label} tone with score {score:F2}. Top phrase '{topBigram}'.",
                "story" =>
                    $"Narrative mode: {label} tone and a crisp lead phrase '{topBigram}'.",
                _ =>
                    $"Balanced mode: {label} tone. ASCII ratio {ratio:F2}. Top phrase '{topBigram}'."
            };

            return new Dictionary<string, object?> { ["text"] = message };
        }
    }
}
