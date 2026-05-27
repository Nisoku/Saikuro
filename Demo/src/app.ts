import { ensureRuntime } from "./lib/runtime";
import { runPipeline, type PipelinePreset } from "./lib/pipeline";
import { MessageLog } from "./lib/message-log";

const DEFAULT_TEXT =
  "Saikuro lets polyglot systems feel like one runtime. This demo pipes the same text through C, C++, Rust, C#, Python, and TypeScript in the browser.";

const PRESETS: PipelinePreset[] = [
  {
    id: "balanced",
    name: "Balanced Insight",
    description: "Default blend of stats, n-grams, sentiment, and summary.",
  },
  {
    id: "precision",
    name: "Precision Analyzer",
    description: "Favor token stats and stable scoring.",
  },
  {
    id: "story",
    name: "Narrative Pulse",
    description: "Emphasize summary and sentiment narrative.",
  },
];

const LANGUAGE_CARDS = [
  {
    name: "C (WASM)",
    role: "Byte-level character stats",
    source: "Demo/wasm/c/insight_c.c",
    snippet: "const char* insight_c_stats(const char* input) {\n  // counts bytes, ascii, non-ascii\n}",
  },
  {
    name: "C++ (WASM)",
    role: "Tokenization + n-gram frequency",
    source: "Demo/wasm/cpp/insight_cpp.cpp",
    snippet: "std::string insight_cpp_ngrams(const std::string& text, int topN)",
  },
  {
    name: "Rust (WASM)",
    role: "Sentiment scoring and tags",
    source: "Demo/wasm/rust/src/lib.rs",
    snippet: "provider.register(\"sentiment\", |args| async move { ... })",
  },
  {
    name: "C# (WASM)",
    role: "Business logic summary",
    source: "Demo/wasm/csharp/InsightLab/Summarizer.cs",
    snippet: "[JSExport] public static string Summarize(string json)",
  },
  {
    name: "Python (Pyodide)",
    role: "Visualization preparation",
    source: "Demo/wasm/python/insight.py",
    snippet: "def prepare_viz(stats, ngrams, sentiment):\n    return {\"bins\": ... }",
  },
  {
    name: "TypeScript (Vite)",
    role: "Orchestration + UI + Saikuro client",
    source: "Demo/src/lib/pipeline.ts",
    snippet: "await client.call(\"c.stats\", [text])",
  },
];

export function bootstrapApp(): void {
  const root = document.querySelector<HTMLDivElement>("#app");
  if (!root) return;

  root.innerHTML = buildAppHtml();

  const textArea = root.querySelector<HTMLTextAreaElement>("#input-text");
  const presetSelect = root.querySelector<HTMLSelectElement>("#preset");
  const runButton = root.querySelector<HTMLButtonElement>("#run-btn");
  const statusEl = root.querySelector<HTMLDivElement>("#status");

  if (!textArea || !presetSelect || !runButton || !statusEl) return;

  textArea.value = DEFAULT_TEXT;
  for (const preset of PRESETS) {
    const option = document.createElement("option");
    option.value = preset.id;
    option.textContent = `${preset.name} - ${preset.description}`;
    presetSelect.appendChild(option);
  }
  presetSelect.value = PRESETS[0].id;

  const messageLog = new MessageLog();

  const updateStatus = (text: string) => {
    statusEl.textContent = text;
  };

  const updateInspector = () => {
    const list = root.querySelector<HTMLDivElement>("#message-list");
    if (!list) return;
    const items = messageLog.list();
    list.innerHTML = items
      .map(
        (item) => `
        <div class="message">
          <div><strong>${item.stage}</strong> - ${item.language} - ${item.direction}</div>
          <div class="status">${item.kind}</div>
          <pre>${item.serialized}</pre>
        </div>
      `,
      )
      .join("");
  };

  const updateGraph = (steps: Array<{ label: string; durationMs: number }>) => {
    const graph = root.querySelector<HTMLDivElement>("#graph");
    const list = root.querySelector<HTMLDivElement>("#step-list");
    if (!graph || !list) return;

    const width = 520;
    const height = 120;
    const gap = width / (steps.length + 1);

    const circles = steps
      .map((step, index) => {
        const x = gap * (index + 1);
        const y = height / 2;
        return `<circle cx="${x}" cy="${y}" r="18" fill="rgba(73,198,178,0.3)" stroke="#49c6b2" />`;
      })
      .join("");

    const labels = steps
      .map((step, index) => {
        const x = gap * (index + 1);
        const y = height / 2 + 36;
        return `<text x="${x}" y="${y}" fill="#9fb2bf" font-size="10" text-anchor="middle">${step.label}</text>`;
      })
      .join("");

    const links = steps
      .slice(1)
      .map((_, index) => {
        const x1 = gap * (index + 1) + 18;
        const x2 = gap * (index + 2) - 18;
        const y = height / 2;
        return `<line x1="${x1}" y1="${y}" x2="${x2}" y2="${y}" stroke="rgba(249,178,51,0.5)" stroke-width="2" />`;
      })
      .join("");

    graph.innerHTML = `
      <svg viewBox="0 0 ${width} ${height}" role="img" aria-label="Pipeline graph">
        ${links}
        ${circles}
        ${labels}
      </svg>
    `;

    list.innerHTML = steps
      .map(
        (step) => `
        <div class="step">
          <div>${step.label}</div>
          <div class="pill">${step.durationMs.toFixed(1)} ms</div>
        </div>
      `,
      )
      .join("");
  };

  const updateResults = (result: any) => {
    const statsEl = root.querySelector<HTMLDivElement>("#stats");
    const ngramsEl = root.querySelector<HTMLUListElement>("#ngrams");
    const sentimentEl = root.querySelector<HTMLDivElement>("#sentiment");
    const summaryEl = root.querySelector<HTMLDivElement>("#summary");
    const vizEl = root.querySelector<HTMLUListElement>("#viz");

    if (statsEl) {
      statsEl.innerHTML = `
        <div class="kv"><span>Bytes</span>${result.stats.bytes}</div>
        <div class="kv"><span>Chars</span>${result.stats.chars}</div>
        <div class="kv"><span>ASCII</span>${result.stats.ascii}</div>
        <div class="kv"><span>Non-ASCII</span>${result.stats.non_ascii}</div>
      `;
    }

    if (ngramsEl) {
      ngramsEl.innerHTML = result.ngrams.bigrams
        .map(
          (item: [string, number]) =>
            `<li>${item[0]} <span class="pill">${item[1]}</span></li>`,
        )
        .join("");
    }

    if (sentimentEl) {
      sentimentEl.innerHTML = `
        <div class="kv"><span>Sentiment</span>${result.sentiment.label}</div>
        <div class="kv"><span>Score</span>${result.sentiment.score.toFixed(2)}</div>
        <div class="kv"><span>Confidence</span>${result.sentiment.confidence.toFixed(2)}</div>
      `;
    }

    if (summaryEl) {
      summaryEl.textContent = result.summary.text;
    }

    if (vizEl) {
      vizEl.innerHTML = result.viz.bins
        .map(
          (item: { label: string; value: number }) =>
            `<li>${item.label} <span class="pill">${item.value}</span></li>`,
        )
        .join("");
    }
  };

  const updateLanguages = () => {
    const container = root.querySelector<HTMLDivElement>("#language-grid");
    if (!container) return;
    container.innerHTML = LANGUAGE_CARDS.map(
      (lang) => `
        <div class="language-card">
          <h3>${lang.name}</h3>
          <small>${lang.role} - ${lang.source}</small>
          <pre>${lang.snippet}</pre>
        </div>
      `,
    ).join("");
  };

  updateLanguages();

  runButton.addEventListener("click", async () => {
    runButton.disabled = true;
    messageLog.clear();
    updateInspector();
    updateStatus("Booting runtime...");

    const preset = PRESETS.find((p) => p.id === presetSelect.value) ?? PRESETS[0];

    try {
      const runtime = await ensureRuntime();
      updateStatus("Running pipeline...");
      const result = await runPipeline(textArea.value.trim(), preset, runtime, messageLog);
      updateResults(result.outputs);
      updateGraph(result.steps.map((step) => ({ label: step.label, durationMs: step.durationMs })));
      updateInspector();
      updateStatus("Pipeline complete.");
    } catch (err) {
      updateStatus(`Pipeline failed: ${String(err)}`);
    } finally {
      runButton.disabled = false;
    }
  });
}

function buildAppHtml(): string {
  return `
    <header class="hero">
      <div>
        <h1>Polyglot Insight Lab</h1>
        <p>One Saikuro runtime, six languages, and a live pipeline you can poke. Each step is a real module compiled to WASM and wired through Saikuro messaging.</p>
      </div>
      <div class="badge-row">
        <div class="badge">Runtime: WASM</div>
        <div class="badge">Transport: wasm-host</div>
        <div class="badge">Pipeline: Live</div>
      </div>
    </header>

    <main class="grid">
      <section class="panel">
        <h2>Input + Preset</h2>
        <textarea id="input-text"></textarea>
        <div style="height: 10px"></div>
        <select id="preset"></select>
        <div style="height: 12px"></div>
        <button id="run-btn">Run Insight Pipeline</button>
        <div style="height: 12px"></div>
        <div class="status" id="status">Idle.</div>
      </section>

      <section class="panel">
        <h2>Pipeline Graph</h2>
        <div class="graph" id="graph"></div>
        <div class="step-list" id="step-list"></div>
      </section>

      <section class="panel">
        <h2>Character Stats (C)</h2>
        <div class="kv-grid" id="stats"></div>
      </section>

      <section class="panel">
        <h2>Top Bigrams (C++)</h2>
        <ul class="list" id="ngrams"></ul>
      </section>

      <section class="panel">
        <h2>Sentiment (Rust)</h2>
        <div class="kv-grid" id="sentiment"></div>
      </section>

      <section class="panel">
        <h2>Summary (C#)</h2>
        <div id="summary" class="status"></div>
      </section>

      <section class="panel">
        <h2>Viz Prep (Python)</h2>
        <ul class="list" id="viz"></ul>
      </section>

      <section class="panel">
        <h2>Message Inspector</h2>
        <div id="message-list" class="graph"></div>
      </section>

      <section class="panel">
        <h2>Language Detail</h2>
        <div id="language-grid" class="language-grid"></div>
      </section>
    </main>
  `;
}
