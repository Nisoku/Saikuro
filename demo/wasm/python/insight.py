"""
Saikuro provider for Python WASM (Pyodide).

Uses the saikuro Python adapter for transport, schema announcement,
message dispatch, and error propagation.
"""

import asyncio
import sys

from saikuro import SaikuroProvider


def prepare_viz(stats, ngrams, sentiment):
    tokens = ngrams.get("bigrams", [])
    token_lengths = []
    for item in tokens:
        words = item[0].split() if isinstance(item, (list, tuple)) and len(item) > 0 else []
        token_lengths.append(max(len(w) for w in words) if words else 0)

    buckets = {"short": 0, "medium": 0, "long": 0}
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


async def main():
    channel = sys.argv[1] if len(sys.argv) > 1 else "saikuro-insight-lab"
    address = f"wasm-host://{channel}"

    provider = SaikuroProvider("python")

    @provider.register("viz")
    async def viz(stats, ngrams, sentiment):
        return prepare_viz(stats, ngrams, sentiment)

    # Run serve in the background so this coroutine completes and
    # pyodide.runPythonAsync() returns.  The provider loop continues
    # on the asyncio event loop.
    asyncio.ensure_future(provider.serve(address))


if __name__ == "__main__":
    try:
        asyncio.get_running_loop()
    except RuntimeError:
        asyncio.run(main())
    else:
        await main()
