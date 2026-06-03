import { getLogger } from "@nisoku/saikuro";
import type { SaikuroClient } from "@nisoku/saikuro";
import type { MessageLog } from "./message-log";

const log = getLogger("demo.pipeline");

export type PipelinePreset = {
  id: string;
  name: string;
  description: string;
};

export type PipelineStep = {
  label: string;
  language: string;
  durationMs: number;
};

export type PipelineOutputs = {
  stats: {
    bytes: number;
    chars: number;
    ascii: number;
    non_ascii: number;
  };
  ngrams: {
    bigrams: Array<[string, number]>;
    trigrams: Array<[string, number]>;
  };
  sentiment: {
    label: string;
    score: number;
    confidence: number;
    tags: string[];
  };
  summary: {
    text: string;
  };
  viz: {
    bins: Array<{ label: string; value: number }>;
  };
};

export type PipelineResult = {
  steps: PipelineStep[];
  outputs: PipelineOutputs;
};

export type RuntimeContext = {
  client: SaikuroClient;
};

type StepResult<T> = {
  result: T;
  durationMs: number;
};

export async function runPipeline(
  input: string,
  preset: PipelinePreset,
  runtime: RuntimeContext,
  messageLog: MessageLog,
): Promise<PipelineResult> {
  log.info("runPipeline start", {
    inputLen: input.length,
    preset: preset.id,
  });

  const steps: PipelineStep[] = [];
  const now = () => performance.now();

  const stats = await callStep<PipelineOutputs["stats"]>({
    client: runtime.client,
    log: messageLog,
    stage: "C Stats",
    language: "C",
    target: "c.stats",
    args: [input],
    preset,
    start: now,
  });
  log.info("C stats complete", { durationMs: stats.durationMs });
  steps.push({ label: "C", language: "C", durationMs: stats.durationMs });

  const ngrams = await callStep<PipelineOutputs["ngrams"]>({
    client: runtime.client,
    log: messageLog,
    stage: "C++ NGrams",
    language: "C++",
    target: "cpp.ngrams",
    args: [input, 6],
    preset,
    start: now,
  });
  log.info("C++ ngrams complete", { durationMs: ngrams.durationMs });
  steps.push({ label: "C++", language: "C++", durationMs: ngrams.durationMs });

  const sentiment = await callStep<PipelineOutputs["sentiment"]>({
    client: runtime.client,
    log: messageLog,
    stage: "Rust Sentiment",
    language: "Rust",
    target: "rust.sentiment",
    args: [input],
    preset,
    start: now,
  });
  log.info("Rust sentiment complete", { durationMs: sentiment.durationMs });
  steps.push({ label: "Rust", language: "Rust", durationMs: sentiment.durationMs });

  const summary = await callStep<PipelineOutputs["summary"]>({
    client: runtime.client,
    log: messageLog,
    stage: "C# Summary",
    language: "C#",
    target: "csharp.summary",
    args: [
      {
        preset: preset.id,
        stats: stats.result,
        ngrams: ngrams.result,
        sentiment: sentiment.result,
      },
    ],
    preset,
    start: now,
  });
  log.info("C# summary complete", { durationMs: summary.durationMs });
  steps.push({ label: "C#", language: "C#", durationMs: summary.durationMs });

  const viz = await callStep<PipelineOutputs["viz"]>({
    client: runtime.client,
    log: messageLog,
    stage: "Python Viz",
    language: "Python",
    target: "python.viz",
    args: [stats.result, ngrams.result, sentiment.result],
    preset,
    start: now,
  });
  log.info("Python viz complete", { durationMs: viz.durationMs });
  steps.push({ label: "Python", language: "Python", durationMs: viz.durationMs });

  log.info("runPipeline complete");
  return {
    steps,
    outputs: {
      stats: stats.result,
      ngrams: ngrams.result,
      sentiment: sentiment.result,
      summary: summary.result,
      viz: viz.result,
    },
  };
}

async function callStep<T>(params: {
  client: SaikuroClient;
  log: MessageLog;
  stage: string;
  language: string;
  target: string;
  args: unknown[];
  preset: PipelinePreset;
  start: () => number;
}): Promise<StepResult<T>> {
  const { client, log, stage, language, target, args, start } = params;
  const id = crypto.randomUUID();
  const callTime = start();
  log.add({
    id,
    stage,
    language,
    direction: "call",
    kind: target,
    serialized: JSON.stringify({ target, args }, null, 2),
    timestamp: callTime,
  });

  const result = (await client.call(target, args)) as T;
  const durationMs = start() - callTime;
  log.add({
    id,
    stage,
    language,
    direction: "response",
    kind: target,
    serialized: JSON.stringify({ result }, null, 2),
    timestamp: start(),
  });

  return { result, durationMs };
}
