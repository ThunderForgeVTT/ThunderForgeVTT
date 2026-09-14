#!/usr/bin/env node
/**
 * Which e2e tests fail in more than one recent run?
 *
 *     node scripts/e2e-flakes.mjs            # the last 10 runs
 *     node scripts/e2e-flakes.mjs --runs=20  # the last 20
 *     node scripts/e2e-flakes.mjs --min=3    # failing in at least 3 of them
 *
 * Reads `.e2e-history.jsonl`, which every `scripts/e2e-parallel.mjs` run
 * appends to. A test counts once per run in which it failed or was flaky. Runs
 * the harness judged busy (see `scripts/e2e/load.mjs`) are counted but marked,
 * because a failure that only ever happens on a busy machine is a different
 * finding from one that also happens on a quiet one.
 */

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(fileURLToPath(import.meta.url), "..", "..");
let HISTORY = join(ROOT, ".e2e-history.jsonl");

let runs = 10;
let min = 2;
for (const arg of process.argv.slice(2)) {
  const r = /^--runs=(\d+)$/.exec(arg);
  const m = /^--min=(\d+)$/.exec(arg);
  const h = /^--history=(.+)$/.exec(arg);
  if (r) runs = Number(r[1]);
  else if (m) min = Number(m[1]);
  else if (h) HISTORY = h[1];
  else {
    process.stderr.write(
      `usage: e2e-flakes.mjs [--runs=N] [--min=K] [--history=FILE]\n`,
    );
    process.exit(2);
  }
}

let entries;
try {
  entries = readFileSync(HISTORY, "utf-8")
    .split("\n")
    .filter(Boolean)
    .flatMap((line) => {
      try {
        return [JSON.parse(line)];
      } catch {
        return [];
      }
    });
} catch {
  process.stdout.write(`No history yet (${HISTORY} does not exist).\n`);
  process.exit(0);
}

const window = entries.slice(-runs);
const tests = new Map();
for (const [position, run] of window.entries()) {
  const seen = new Set();
  const note = (item, kind) => {
    const key = `${item.spec} › ${item.title}`;
    if (seen.has(key)) return;
    seen.add(key);
    const entry = tests.get(key) ?? {
      key,
      failed: 0,
      flaky: 0,
      busy: 0,
      timeouts: 0,
      last: null,
      runs: [],
    };
    entry[kind] += 1;
    if (run.busy) entry.busy += 1;
    if (item.timeout) entry.timeouts += 1;
    entry.last = run.startedAt;
    entry.runs.push(position + 1);
    tests.set(key, entry);
  };
  for (const item of run.failures ?? []) note(item, "failed");
  for (const item of run.flaky ?? []) note(item, "flaky");
}

const repeat = [...tests.values()]
  .filter((t) => t.failed + t.flaky >= min)
  .sort((a, b) => b.failed + b.flaky - (a.failed + a.flaky));

const busyRuns = window.filter((r) => r.busy).length;
process.stdout.write(
  `${window.length} run(s) considered (${busyRuns} busy); ` +
    `tests failing or flaky in at least ${min}:\n`,
);
if (repeat.length === 0) {
  process.stdout.write("  none.\n");
  process.exit(0);
}
for (const t of repeat) {
  const parts = [`${t.failed} failed`, `${t.flaky} flaky`];
  if (t.timeouts) parts.push(`${t.timeouts} timed out`);
  if (t.busy) parts.push(`${t.busy} on a busy machine`);
  process.stdout.write(
    `  ${t.failed + t.flaky}/${window.length}  ${t.key}\n` +
      `        ${parts.join(", ")}; runs #${t.runs.join(", #")} of the window; last ${t.last}\n`,
  );
}
