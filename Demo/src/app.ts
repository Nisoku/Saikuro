import { getLogger } from "@nisoku/saikuro";
import { ensureRuntime } from "./lib/runtime";
import { runPipeline, type PipelineOutputs, type PipelinePreset, type PipelineResult } from "./lib/pipeline";
import { MessageLog, type MessageRecord } from "./lib/message-log";

const log = getLogger("demo.app");

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

/** Map a PipelineStep label to a flow segment data-stage key. */
const STAGE_TO_KEY: Record<string, string> = {
  C: "C",
  "C++": "C++",
  Rust: "Rust",
  "C#": "C#",
  Python: "Python",

};

/** Map a pipeline stage name to a CSS modifier for the log rail badge. */
const STAGE_TO_RAIL_CLASS: Record<string, string> = {
  "C Stats": "c",
  "C++ NGrams": "cpp",
  "Rust Sentiment": "rust",
  "C# Summary": "cs",
  "Python Viz": "py",
};

/** Map a pipeline step label to a provider card data-provider attribute. */
const STEP_TO_PROVIDER: Record<string, string> = {
  C: "C",
  "C++": "C++",
  Rust: "Rust",
  "C#": "C#",
  Python: "Python",
};

/** A short, single-line payload preview for the log rail. */
function shortPreview(s: string, max = 64): string {
  const trimmed = s.replace(/\s+/g, " ").trim();
  return trimmed.length > max ? trimmed.slice(0, max - 1) + "\u2026" : trimmed;
}

/** Format a number with thousands separators. */
function formatNumber(n: number): string {
  return new Intl.NumberFormat("en-US").format(n);
}

/** Minimal HTML-escape for user-derived strings injected into the DOM. */
const AMP = String.fromCharCode(38) + "amp;";
const LT = String.fromCharCode(38) + "lt;";
const GT = String.fromCharCode(38) + "gt;";
const QUOT = String.fromCharCode(38) + "quot;";
const APOS = String.fromCharCode(38) + "#39;";

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, AMP)
    .replace(/</g, LT)
    .replace(/>/g, GT)
    .replace(/"/g, QUOT)
    .replace(/'/g, APOS);
}

export function bootstrapApp(): void {
  // DOM references
  const textArea = document.querySelector<HTMLTextAreaElement>("#input-text");
  const presetSelect = document.querySelector<HTMLSelectElement>("#preset");
  const runButton = document.querySelector<HTMLButtonElement>("#run-btn");
  const statusEl = document.querySelector<HTMLSpanElement>("#status-text");
  const logRail = document.querySelector<HTMLDivElement>("#log-rail");
  const flowTrack = document.querySelector<HTMLDivElement>("#flow-track");
  const ngramsCloud = document.querySelector<HTMLDivElement>("#ngrams-cloud");
  const vizBars = document.querySelector<HTMLDivElement>("#viz-bars");
  const gaugeFill = document.querySelector<SVGPathElement>(".gauge__fill");
  const messageList = document.querySelector<HTMLDivElement>("#message-list");
  const ngramsDetailBigrams = document.querySelector<HTMLDivElement>("#ngrams-detail-bigrams");
  const ngramsDetailTrigrams = document.querySelector<HTMLDivElement>("#ngrams-detail-trigrams");

  if (!textArea || !presetSelect || !runButton || !statusEl) return;

  // Initial UI state
  textArea.value = DEFAULT_TEXT;
  for (const preset of PRESETS) {
    const option = document.createElement("option");
    option.value = preset.id;
    option.textContent = preset.name;
    presetSelect.appendChild(option);
  }
  presetSelect.value = PRESETS[0].id;

  const messageLog = new MessageLog();

  // Tab switching
  function switchTab(tabName: string) {
    document.querySelectorAll<HTMLAnchorElement>("#sidebar-nav .nav-item").forEach((item) => {
      item.classList.toggle("is-active", item.dataset.tab === tabName);
    });
    document.querySelectorAll<HTMLAnchorElement>("#bottom-nav .bottom-nav__item").forEach((item) => {
      item.classList.toggle("is-active", item.dataset.tab === tabName);
    });
    document.querySelectorAll<HTMLElement>("[data-view]").forEach((view) => {
      view.classList.toggle("is-active", view.dataset.view === tabName);
    });
    const main = document.querySelector<HTMLElement>(".main");
    if (main) main.scrollTop = 0;
  }

  document.querySelectorAll<HTMLElement>("[data-tab]").forEach((el) => {
    el.addEventListener("click", (e) => {
      e.preventDefault();
      switchTab(el.dataset.tab ?? "dashboard");
    });
  });

  // Log rail (footer)
  const renderLogRail = (limit = 6) => {
    if (!logRail) return;
    const items = messageLog.list().slice(0, limit);
    if (items.length === 0) {
      logRail.innerHTML = '<span class="log-rail__placeholder">No messages yet.</span>';
      return;
    }
    logRail.innerHTML = items
      .map((item) => {
        const cls = STAGE_TO_RAIL_CLASS[item.stage] ?? "";
        const stageLabel = escapeHtml(item.stage.replace(/\s+/g, "-"));
        return '<div class="log-rail__item">' +
          '<span class="log-rail__stage log-rail__stage--' + cls + '">' + stageLabel + "</span>" +
          '<span class="log-rail__payload">' + escapeHtml(shortPreview(item.serialized)) + "</span>" +
          "</div>";
      })
      .join("");
  };

  // Logs view (full message list)
  const renderLogsView = () => {
    if (!messageList) return;
    const items = messageLog.list();
    if (items.length === 0) {
      messageList.innerHTML = '<div style="padding:24px;text-align:center;color:#718096;">No messages yet. Run the pipeline to see log entries.</div>';
      return;
    }
    messageList.innerHTML = items
      .map((item) => {
        const dir = item.direction === "call" ? "call" : "response";
        return '<div class="log-entry">' +
          '<span class="log-entry__badge log-entry__badge--' + dir + '">' + escapeHtml(item.stage) + " " + dir + "</span>" +
          '<code class="log-entry__data">' + escapeHtml(shortPreview(item.serialized, 200)) + "</code>" +
          "</div>";
      })
      .join("");
  };

  // Execution flow segments
  const renderFlow = (steps: PipelineResult["steps"]) => {
    if (!flowTrack) return;
    const total = steps.reduce((acc, s) => acc + s.durationMs, 0) || 1;
    flowTrack.querySelectorAll<HTMLDivElement>(".flow__segment").forEach((seg) => {
      const key = seg.dataset.stage ?? "";
      const step = steps.find((s) => STAGE_TO_KEY[s.label] === key);
      if (!step) {
        seg.style.width = "0%";
        return;
      }
      const pct = Math.max(4, (step.durationMs / total) * 100);
      seg.style.width = pct.toFixed(2) + "%";
    });
  };

  // N-Gram tag clouds
  const renderNgrams = (bigrams: PipelineOutputs["ngrams"]["bigrams"], trigrams?: PipelineOutputs["ngrams"]["trigrams"]) => {
    if (ngramsCloud && Array.isArray(bigrams) && bigrams.length > 0) {
      ngramsCloud.innerHTML = bigrams.slice(0, 8)
        .map(([word, count]) => '<span class="tag">' + escapeHtml(word) + " (" + formatNumber(count) + ")</span>")
        .join("");
    }
    if (ngramsDetailBigrams && Array.isArray(bigrams) && bigrams.length > 0) {
      ngramsDetailBigrams.innerHTML = bigrams
        .map(([word, count]) => '<span class="tag">' + escapeHtml(word) + " (" + formatNumber(count) + ")</span>")
        .join("");
    }
    if (ngramsDetailTrigrams && Array.isArray(trigrams) && trigrams.length > 0) {
      ngramsDetailTrigrams.innerHTML = trigrams
        .map(([word, count]) => '<span class="tag">' + escapeHtml(word) + " (" + formatNumber(count) + ")</span>")
        .join("");
    }
  };

  // Viz bar chart
  const renderVizBars = (bins: PipelineOutputs["viz"]["bins"]) => {
    if (!vizBars || !Array.isArray(bins) || bins.length === 0) return;
    const max = Math.max(...bins.map((b) => b.value), 1);
    const track = vizBars.querySelector<HTMLDivElement>(".bars__track");
    const labels = vizBars.querySelector<HTMLDivElement>(".bars__labels");
    if (track) {
      track.innerHTML = bins
        .map((b) => {
          const pct = Math.max(4, (b.value / max) * 100);
          return '<div class="bar" style="height: ' + pct.toFixed(1) + '%" title="' + escapeHtml(b.label) + ": " + b.value + '"></div>';
        })
        .join("");
    }
    if (labels) {
      labels.innerHTML = bins.map((b) => "<span>" + escapeHtml(b.label) + "</span>").join("");
    }
  };

  // Gauge ring
  const renderGauge = (score: number) => {
    if (!gaugeFill) return;
    const clamped = Math.max(0, Math.min(100, score));
    gaugeFill.setAttribute("stroke-dasharray", clamped + ", 100");
  };

  // Provider cards (pipeline view)
  const updateProviderCard = (providerName: string, status: "idle" | "running" | "done" | "error", durationMs?: number) => {
    const card = document.querySelector<HTMLElement>('[data-provider="' + providerName + '"]');
    if (!card) return;
    card.classList.toggle("is-running", status === "running");
    const pill = card.querySelector<HTMLElement>("[data-provider-status]");
    if (pill) {
      const dot = pill.querySelector(".status-dot");
      pill.className = pill.className.replace(/status-pill--\w+/g, "").trim();
      if (dot) dot.className = dot.className.replace(/status-dot--\w+/g, "").trim();
      switch (status) {
        case "running":
          pill.classList.add("status-pill", "status-pill--running");
          if (dot) dot.classList.add("status-dot", "status-dot--pulse");
          pill.childNodes.forEach((n) => { if (n.nodeType === Node.TEXT_NODE) n.textContent = " Running"; });
          break;
        case "done":
          pill.classList.add("status-pill", "status-pill--ready");
          if (dot) dot.classList.add("status-dot", "status-dot--ready");
          pill.childNodes.forEach((n) => { if (n.nodeType === Node.TEXT_NODE) n.textContent = " Idle"; });
          break;
        case "error":
          pill.classList.add("status-pill", "status-pill--error");
          if (dot) dot.classList.add("status-dot");
          pill.childNodes.forEach((n) => { if (n.nodeType === Node.TEXT_NODE) n.textContent = " Error"; });
          break;
        default:
          pill.classList.add("status-pill", "status-pill--ready");
          if (dot) dot.classList.add("status-dot", "status-dot--ready");
          pill.childNodes.forEach((n) => { if (n.nodeType === Node.TEXT_NODE) n.textContent = " Idle"; });
      }
    }
    if (durationMs !== undefined) {
      const dur = card.querySelector<HTMLElement>("[data-provider-duration]");
      if (dur) dur.textContent = Math.round(durationMs) + "ms";
    }
  };

  const resetAllProviderCards = () => {
    document.querySelectorAll<HTMLElement>("[data-provider]").forEach((card) => {
      const name = card.dataset.provider ?? "";
      updateProviderCard(name, "idle");
    });
  };

  // Apply all results
  const applyResults = (result: PipelineResult) => {
    const outputs = result.outputs;
    const steps = result.steps;
    const totalMs = steps.reduce((acc, s) => acc + s.durationMs, 0);

    document.querySelectorAll<HTMLElement>("[data-result]").forEach((el) => {
      const key = el.getAttribute("data-result");
      if (!key) return;
      const path = key.split(".");
      let val: unknown = outputs;
      for (const p of path) {
        const obj = val as Record<string, unknown> | undefined;
        if (obj && typeof obj === "object" && p in obj) {
          val = obj[p];
        } else {
          val = undefined;
          break;
        }
      }

      if (key === "stats.asciiPct") {
        const stats = outputs.stats;
        const denom = stats.ascii + stats.non_ascii;
        const pct = denom > 0 ? (stats.ascii / denom) * 100 : 0;
        el.textContent = pct.toFixed(1) + "%";
        return;
      }

      if (key === "stats.nullTerminated") {
        el.textContent = outputs.stats.non_ascii === 0 ? "True" : "False";
        return;
      }

      if (key === "performance.total" || key === "performance.runtime") {
        el.textContent = Math.round(totalMs) + "ms";
        return;
      }

      if (key === "sentiment.score") {
        const score = outputs.sentiment.score ?? 0;
        el.textContent = Math.round(score) + "%";
        renderGauge(score);
        return;
      }

      if (key === "sentiment.confidence") {
        const conf = outputs.sentiment.confidence;
        el.textContent = typeof conf === "number" ? (conf > 0.7 ? "High" : conf > 0.4 ? "Medium" : "Low") : "\u2014";
        return;
      }

      if (val === undefined || val === null) {
        el.textContent = "\u2014";
      } else if (typeof val === "number") {
        el.textContent = formatNumber(val);
      } else if (typeof val === "object") {
        el.textContent = JSON.stringify(val);
      } else {
        el.textContent = String(val);
      }
    });

    renderFlow(steps);
    renderNgrams(outputs.ngrams.bigrams, outputs.ngrams.trigrams);
    renderVizBars(outputs.viz.bins);

    steps.forEach((step) => {
      const providerName = STEP_TO_PROVIDER[step.label];
      if (providerName) {
        updateProviderCard(providerName, "done", step.durationMs);
      }
    });
  };

  // Run button state
  const setRunning = (running: boolean) => {
    runButton.disabled = running;
    runButton.classList.toggle("is-loading", running);
  };

  // Pipeline execution
  runButton.addEventListener("click", async () => {
    setRunning(true);
    messageLog.clear();
    renderLogRail();
    renderLogsView();
    statusEl.textContent = "Analyzing pipeline...";
    resetAllProviderCards();

    const providerOrder = ["C", "C++", "Rust", "C#", "Python"];
    let providerIdx = 0;
    const interval = setInterval(() => {
      if (providerIdx < providerOrder.length) {
        updateProviderCard(providerOrder[providerIdx], "running");
        if (providerIdx > 0) {
          updateProviderCard(providerOrder[providerIdx - 1], "done");
        }
        providerIdx++;
      }
    }, 200);

    try {
      const runtime = await ensureRuntime();
      const preset = PRESETS.find((p) => p.id === presetSelect.value) ?? PRESETS[0];
      const result = await runPipeline(textArea.value.trim(), preset, runtime, messageLog);

      clearInterval(interval);
      providerOrder.forEach((name) => updateProviderCard(name, "done"));

      applyResults(result);
      renderLogRail();
      renderLogsView();
      statusEl.textContent = "Analysis Complete.";
    } catch (err) {
      clearInterval(interval);
      const message = err instanceof Error ? err.message : String(err);
      log.error("Pipeline failure", { error: message });
      statusEl.textContent = "Pipeline Error.";
      document.querySelectorAll<HTMLElement>(".provider-card.is-running").forEach((card) => {
        const name = card.dataset.provider;
        if (name) updateProviderCard(name, "error");
      });
    } finally {
      setRunning(false);
    }
  });
}

export type { MessageRecord };
