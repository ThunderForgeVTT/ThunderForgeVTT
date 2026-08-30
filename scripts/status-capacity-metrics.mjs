#!/usr/bin/env node
/**
 * Measure the engine's token capacity with and without status displays, and
 * write what it measured to `marketing/`, as JSON.
 *
 *   node scripts/status-capacity-metrics.mjs        # 2 runs
 *   node scripts/status-capacity-metrics.mjs 3      # 3 runs
 *
 * # Why this exists rather than a number written into the spec
 *
 * Spec 029 SC-006 asks for a *stated* capacity cost. `marketing-metrics.mjs`
 * already explains why nothing here is transcribed: this repository has had a
 * performance figure drift 16% while people quoted it. So this parses its
 * figures out of a run that just happened, and records the date, the host, and
 * the command that produced them, so a stale entry is visibly stale rather
 * than quietly wrong.
 *
 * # Why more than one run
 *
 * A single sample stated as a capacity figure is the same unmeasured claim
 * SC-006 forbids, dressed as a measurement. Frame time on a desktop GPU moves
 * between runs; a reader who cannot see that spread cannot tell a real cost
 * from the noise floor. Every run is kept in the file rather than averaged
 * away, and the summary states each run's capacity as well as the conservative
 * reading of the pair.
 *
 * # What is measured
 *
 * `apps/web/e2e/engine-status-limits.spec.ts`, which sweeps token count twice —
 * once with plain tokens that display nothing, once with tokens bound to an
 * actor with resources so every one of them draws bars — and reads the engine's
 * own counters at each level. Both sides are measured in the same session on
 * the same machine, because a cost read off the gap between a fresh number and
 * a committed old one measures the gap between runs, not the feature.
 *
 * The dev stack must already be up (`pnpm dev`); Playwright reuses it.
 */

import { spawn } from "node:child_process";
import { mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import os from "node:os";
import path from "node:path";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const WEB = path.join(ROOT, "apps", "web");
const runs = Number(process.argv[2] ?? 2);
const SPEC = "e2e/engine-status-limits.spec.ts";
const ARGS = ["playwright", "test", SPEC, "--workers=1", "--reporter=line"];

/** One Playwright run; resolves with its exit code and everything it printed. */
function runOnce() {
  return new Promise((resolve) => {
    const child = spawn("npx", ARGS, { cwd: WEB, env: process.env });
    const chunks = [];
    const collect = (buffer) => {
      const text = buffer.toString();
      chunks.push(text);
      process.stderr.write(text);
    };
    child.stdout.on("data", collect);
    child.stderr.on("data", collect);
    child.on("exit", (code) => resolve({ code, output: chunks.join("") }));
  });
}

/**
 * Pull the run's own summary line out of its output.
 *
 * The spec prints `[status-capacity] result={…}` — the same object its
 * assertions are made against, not a second measurement taken for
 * presentation. Anything else in the output is Playwright's, not the engine's.
 */
function parseResult(output) {
  const line = output
    .split("\n")
    .reverse()
    .find((candidate) => candidate.includes("[status-capacity] result="));
  if (!line) return null;
  try {
    return JSON.parse(line.slice(line.indexOf("result=") + "result=".length));
  } catch {
    return null;
  }
}

/**
 * Which engine build this ran against.
 *
 * Not decoration. These figures were first taken against a wasm build that
 * changed under the run — the same board reported five sprites per token in one
 * build and a fraction of that an hour later, because the engine was being
 * edited at the time. A capacity figure with no build behind it cannot be told
 * apart from a stale one, which is the failure this whole file exists to avoid.
 */
function engineBuild() {
  const dir = path.join(WEB, "node_modules", "@thunderforge", "engine");
  try {
    return {
      pkgSum: readFileSync(path.join(dir, "pkg.sum"), "utf8").trim(),
      builtAt: statSync(path.join(dir, "engine_bg.wasm")).mtime.toISOString(),
    };
  } catch {
    return null;
  }
}

const median = (values) => {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
};

const started = new Date();
const results = [];
let failed = false;

for (let i = 0; i < runs; i += 1) {
  process.stderr.write(`\n[status-capacity] run ${i + 1} of ${runs}\n`);
  const { code, output } = await runOnce();
  const parsed = parseResult(output);
  if (code !== 0 || !parsed) failed = true;
  results.push({ run: i + 1, passed: code === 0, ...(parsed ?? {}) });
}

const withFigures = results.filter((result) => result.absent && result.enabled);
const summary = withFigures.length
  ? {
      levels: withFigures[0].levels,
      interactiveFps: withFigures[0].interactiveFps,
      // The capacity each run reported, kept per run rather than reduced to
      // one number: if two runs disagree, that disagreement is the finding.
      capacityAbsentPerRun: withFigures.map((r) => r.capacityAbsent),
      capacityEnabledPerRun: withFigures.map((r) => r.capacityEnabled),
      // The conservative reading of the pair. A capacity claim that only one
      // run supports is not a capacity.
      capacityAbsent: Math.min(...withFigures.map((r) => r.capacityAbsent)),
      capacityEnabled: Math.min(...withFigures.map((r) => r.capacityEnabled)),
      anchor: withFigures[0].anchor,
      anchorFrameTimeMs: {
        absent: withFigures.map(
          (r) => r.absent.find((s) => s.tokens === r.anchor)?.frameTimeMs,
        ),
        enabled: withFigures.map(
          (r) => r.enabled.find((s) => s.tokens === r.anchor)?.frameTimeMs,
        ),
      },
      anchorSpriteDelta: median(withFigures.map((r) => r.anchorSpriteDelta)),
      // Sprites of status geometry per token, at the anchor. The engine spawns
      // a track and a fill for each resource, so a board where every token
      // displays two resources reads as 4. A number well under that means the
      // geometry did not reach every token, and the frame time beside it is
      // therefore a lower bound on the feature's cost rather than its price.
      anchorBarSpritesPerToken: withFigures.map((r) =>
        Number((r.anchorSpriteDelta / r.anchor).toFixed(2)),
      ),
    }
  : null;

const finished = new Date();
const report = {
  // Everything needed to know whether this file still describes reality.
  generatedAt: finished.toISOString(),
  generatedBy: `node scripts/status-capacity-metrics.mjs ${runs}`,
  measures:
    "spec 029 SC-006 — engine token capacity with in-engine status displays " +
    "enabled, against the same board with none",
  passed: !failed,
  durationSeconds: Math.round((finished - started) / 1000),
  engine: engineBuild(),
  host: {
    cpuCores: os.cpus().length,
    totalMemGiB: Number((os.totalmem() / 1024 ** 3).toFixed(1)),
    platform: `${os.type()} ${os.release()}`,
  },
  // Every run, not an average. The spread is the point.
  runs: results,
  summary,
  // Stated in the data rather than in a footnote nobody reads. Frame time here
  // is bounded below by the display's refresh interval: a level that finishes
  // early reports the vsync figure and says nothing about headroom.
  note:
    "frame time is floored and quantised by vsync on this host — a level " +
    "lands on 16.7ms, 33.3ms or 50ms and nothing between — so equal frame " +
    "times mean both conditions finished inside the same refresh interval, " +
    "not that the work was identical. Capacity is read as the largest level " +
    "still holding the interactive frame rate.",
  profile:
    "vite dev server and a debug backend; the engine wasm is the built pkg",
};

mkdirSync(path.join(ROOT, "marketing"), { recursive: true });
const file = path.join(ROOT, "marketing", "engine-status-capacity.json");
writeFileSync(file, `${JSON.stringify(report, null, 2)}\n`);
process.stdout.write(`\n[status-capacity] wrote ${path.relative(ROOT, file)}\n`);
process.exit(failed ? 1 : 0);
