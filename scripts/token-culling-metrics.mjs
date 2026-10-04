#!/usr/bin/env node
/**
 * Measure what viewport culling of token furniture saves, and write what it
 * measured to `marketing/`, as JSON.
 *
 *   node scripts/token-culling-metrics.mjs        # 2 runs
 *   node scripts/token-culling-metrics.mjs 3      # 3 runs
 *
 * A sibling of `status-capacity-metrics.mjs`, and the same shape for the same
 * reasons, which that file gives at length: nothing here is transcribed, every
 * figure is parsed out of a run that just happened, and the date, the host,
 * the engine build and the command are recorded beside it so a stale entry is
 * visibly stale rather than quietly wrong. Every run is kept rather than
 * averaged away, because the spread between runs is part of the finding.
 *
 * # What is measured
 *
 * `apps/web/e2e/engine-token-culling.spec.ts`: one board of tokens that all
 * draw bars, sampled in one page with culling on and with it off — same
 * tokens, same camera, same build, a few seconds apart — plus a walk of pans
 * that checks no token inside the padded view is ever missing its furniture.
 * `TOKEN_CULLING_TOKENS` changes the board's size; the size that ran is in
 * the file, and a figure quoted anywhere is from the default.
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
const SPEC = "e2e/engine-token-culling.spec.ts";
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
 * The spec prints `[token-culling] result={…}` — the same object its
 * assertions are made against, not a second measurement taken for
 * presentation. Anything else in the output is Playwright's, not the engine's.
 */
function parseResult(output) {
  const line = output
    .split("\n")
    .reverse()
    .find((candidate) => candidate.includes("[token-culling] result="));
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
  process.stderr.write(`\n[token-culling] run ${i + 1} of ${runs}\n`);
  const { code, output } = await runOnce();
  const parsed = parseResult(output);
  if (code !== 0 || !parsed) failed = true;
  results.push({ run: i + 1, passed: code === 0, ...(parsed ?? {}) });
}

const withFigures = results.filter((result) => result.on && result.off);
const summary = withFigures.length
  ? {
      tokens: withFigures[0].tokens,
      spacing: withFigures[0].spacing,
      viewPad: withFigures[0].viewPad,
      // Per run rather than reduced to one number: if two runs disagree, that
      // disagreement is the finding.
      inViewAtRestPerRun: withFigures.map((r) => r.inViewAtRest),
      spritesPerRun: {
        on: withFigures.map((r) => r.on.sprites),
        off: withFigures.map((r) => r.off.sprites),
      },
      // The engine's own count of culled tokens, beside the bars actually
      // built: two readings of one fact that should move together.
      tokensCulledPerRun: {
        on: withFigures.map((r) => r.on.culled),
        off: withFigures.map((r) => r.off.culled),
      },
      tokensWithBarsPerRun: {
        on: withFigures.map((r) => r.on.withBars),
        off: withFigures.map((r) => r.off.withBars),
      },
      frameTimeMsPerRun: {
        on: withFigures.map((r) => r.on.frameTimeMs),
        off: withFigures.map((r) => r.off.frameTimeMs),
      },
      fpsPerRun: {
        on: withFigures.map((r) => r.on.fps),
        off: withFigures.map((r) => r.off.fps),
      },
      spritesSaved: median(withFigures.map((r) => r.spritesSaved)),
      // The conservative reading. A saving only one run supports is not one.
      frameTimeSavedMs: Math.min(...withFigures.map((r) => r.frameTimeSavedMs)),
      // How many stops the camera made, and that each found tokens to check.
      pansPerRun: withFigures.map((r) => r.pans.length),
      fewestInViewAfterAPan: Math.min(
        ...withFigures.flatMap((r) => r.pans.map((pan) => pan.inView)),
      ),
    }
  : null;

const finished = new Date();
const report = {
  // Everything needed to know whether this file still describes reality.
  generatedAt: finished.toISOString(),
  generatedBy: `node scripts/token-culling-metrics.mjs ${runs}`,
  measures:
    "viewport culling of token names and bars — the same board, camera and " +
    "session with culling on against culling off",
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
  // Stated in the data rather than in a footnote nobody reads.
  note:
    "frame time is floored and quantised by vsync on this host — a reading " +
    "lands on 16.7ms, 33.3ms or 50ms and nothing between — so equal frame " +
    "times with culling on and off mean both finished inside the same " +
    "refresh interval, not that culling saved nothing. The sprite and bar " +
    "counts are exact; they are entities the engine holds, not an estimate.",
  profile:
    "vite dev server and a debug backend; the engine wasm is the built pkg",
};

mkdirSync(path.join(ROOT, "marketing"), { recursive: true });
const file = path.join(ROOT, "marketing", "token-culling.json");
writeFileSync(file, `${JSON.stringify(report, null, 2)}\n`);
process.stdout.write(`\n[token-culling] wrote ${path.relative(ROOT, file)}\n`);
process.exit(failed ? 1 : 0);
