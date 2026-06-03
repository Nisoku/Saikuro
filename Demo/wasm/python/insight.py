def prepare_viz(stats, ngrams, sentiment):
    tokens = ngrams.get("bigrams", [])
    token_lengths = [max(len(w) for w in item[0].split()) for item in tokens]
    buckets = {
        "short": 0,
        "medium": 0,
        "long": 0,
    }
    for length in token_lengths:
        if length <= 6:
            buckets["short"] += 1
        elif length <= 12:
            buckets["medium"] += 1
        else:
            buckets["long"] += 1

    bins = [
        {"label": "short", "value": buckets["short"]},
        {"label": "medium", "value": buckets["medium"]},
        {"label": "long", "value": buckets["long"]},
    ]

    return {
        "bins": bins,
        "sentiment": sentiment.get("label", "neutral"),
        "ascii_ratio": 0 if stats.get("bytes", 0) == 0 else stats.get("ascii", 0) / stats.get("bytes", 1),
    }
