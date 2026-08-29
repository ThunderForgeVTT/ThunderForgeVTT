#!/usr/bin/env node
/**
 * Run a load tier and write what it measured to `marketing/`, as JSON.
 *
 *   node scripts/marketing-metrics.mjs 25
 *
 * # Why this exists rather than someone copying numbers into a slide
 *
 * This repository already has one performance figure that drifted: `MVP.md`
 * carried "~190MB (confirmed)" for the engine bundle while the real value had
 * reached 220MB — 16% wrong, unnoticed, in a document people quoted. A number
 * repeated by hand is a number that goes stale the first time the code moves,
 * and nobody finds out because nothing checks it.
 *
 * So nothing here is transcribed. Every figure is parsed out of a run that
 * just happened, and every file records the date, the host it ran on, and the
 * exact command that produced it — so a stale entry is visibly stale rather
 * than quietly wrong.
 *
 * # What is measured, and the honesty that costs
 *
 * Two things at once:
 *
 * 1. **What the application did** — the `[torture]` summary lines each
 *    scenario prints. These are the same lines the assertions are made
 *    against, not a second measurement taken for presentation.
 * 2. **What it cost the machine** — `docker stats` sampled across the run's
 *    own throwaway containers.
 *
 * The container figures are Postgres and the object store only. The backend
 * runs on the host under `cargo run`, not in a container, so its own CPU and
 * memory are **not** in these numbers and the JSON says so in a field rather
 * than in a footnote nobody reads. Claiming otherwise would be the same
 * mistake as the bundle figure, made deliberately.
 */

import { spawn, execFile } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import os from "node:os";
import path from "node:path";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const tier = process.argv[2] ?? "25";

/** One `docker stats` reading for the containers this run created.
 *
 * Asynchronous, and that is not a style choice.
 *
 * This was `execFileSync`, on a 2s interval, in the same process that drains
 * the run's stdout and stderr through a pipe. `docker stats --no-stream`
 * takes the better part of a second, and for every one of those seconds this
 * event loop did not read the pipe. A pipe holds 64KiB; the server on the
 * other end writes to it with a blocking `write(2)`, so once it filled, the
 * server's own threads stopped — inside the tasks carrying live subscriptions.
 *
 * The measurement was destroying the thing it measured, and doing it
 * invisibly: the same tier run without this wrapper passed 5/5 in 3 minutes,
 * and through it lost 11 of 25 subscribers and took 9.5. A load harness that
 * perturbs the load is worse than no harness, because its numbers look real.
 */
function sampleContainers() {
  return new Promise((resolve) => {
    execFile(
      "docker",
      [
        "stats",
        "--no-stream",
        "--format",
        "{{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}",
      ],
      { encoding: "utf8" },
      (error, out) => {
        if (error) {
          // Docker not answering is not a reason to lose the run's own results.
          resolve([]);
          return;
        }
        const rows = [];
        for (const line of out.split("\n")) {
          const [name, cpu, mem] = line.split("\t");
          // Only this harness's own containers. The dev stack's Postgres has the
          // same image and a very similar name, and folding it in would report a
          // number nobody could reproduce.
          if (!name?.startsWith("tf-torture-")) continue;
          rows.push({
            name,
            cpuPercent: Number.parseFloat(cpu),
            memMiB: Number.parseFloat(mem),
          });
        }
        resolve(rows);
      },
    );
  });
}

const peakByContainer = new Map();
function recordPeaks(rows) {
  for (const row of rows) {
    const seen = peakByContainer.get(row.name) ?? { cpuPercent: 0, memMiB: 0 };
    peakByContainer.set(row.name, {
      cpuPercent: Math.max(seen.cpuPercent, row.cpuPercent || 0),
      memMiB: Math.max(seen.memMiB, row.memMiB || 0),
    });
  }
}

/** Parse the `[torture] key=value ...` lines each scenario prints. */
function parseScenarios(output) {
  const scenarios = [];
  for (const line of output.split("\n")) {
    const match = /\[torture\]\s+(.*)$/.exec(line.replace(/\[[0-9;]*m/g, ""));
    if (!match) continue;
    const body = match[1].trim();
    // Only the summary lines, which are all `key=value` pairs. The runner's
    // own prose lines ("starting the backend") are not results.
    if (!/^[a-zA-Z]+=/.test(body)) continue;
    const fields = {};
    for (const pair of body.split(/\s+/)) {
      const [key, value] = pair.split("=");
      if (key && value !== undefined) {
        const asNumber = Number(value);
        fields[key] = Number.isFinite(asNumber) ? asNumber : value;
      }
    }
    scenarios.push(fields);
  }
  return scenarios;
}

const started = new Date();
const child = spawn("node", [path.join(ROOT, "scripts", "torture.mjs"), tier], {
  cwd: ROOT,
  env: { ...process.env, CONFIRM: "1" },
  stdio: ["ignore", "pipe", "pipe"],
});

// Collected in pieces rather than one growing string: `output += chunk` is
// a fresh copy of everything so far on every chunk, and a tier-100 run emits
// enough of them for that to become the reason the pipe is not being read.
const output = [];
child.stdout.on("data", (chunk) => {
  const text = String(chunk);
  output.push(text);
  process.stdout.write(text);
});
child.stderr.on("data", (chunk) => {
  const text = String(chunk);
  output.push(text);
  process.stderr.write(text);
});

// One sample in flight at a time. Overlapping `docker stats` calls would
// queue up behind each other and turn a sampler into a second workload.
let sampling = false;
const sampler = setInterval(() => {
  if (sampling) return;
  sampling = true;
  sampleContainers()
    .then(recordPeaks)
    .finally(() => {
      sampling = false;
    });
}, 2_000);

child.on("exit", (code) => {
  clearInterval(sampler);
  const finished = new Date();

  const scenarios = parseScenarios(output.join(""));
  const containers = [...peakByContainer.entries()].map(([name, peak]) => ({
    // The random run id makes the raw name useless for comparing two runs.
    role: name.includes("postgres") ? "postgres" : "object-store",
    peakCpuPercent: Number(peak.cpuPercent.toFixed(2)),
    peakMemMiB: Number(peak.memMiB.toFixed(1)),
  }));

  const report = {
    // Everything needed to know whether this file still describes reality.
    generatedAt: finished.toISOString(),
    generatedBy: `node scripts/marketing-metrics.mjs ${tier}`,
    passed: code === 0,
    tier: Number(tier),
    durationSeconds: Math.round((finished - started) / 1000),
    host: {
      cpuCores: os.cpus().length,
      totalMemGiB: Number((os.totalmem() / 1024 ** 3).toFixed(1)),
      platform: `${os.type()} ${os.release()}`,
    },
    scenarios,
    containers,
    // Stated in the data, not in a footnote. The backend runs on the host
    // under `cargo run`, so its CPU and memory are not in `containers` — a
    // reader who assumes otherwise would be overstating how little this
    // costs, which is exactly the kind of error that is hard to walk back.
    containerScope:
      "postgres and object store only; the backend runs on the host under cargo run and is not included",
    profile: "debug (cargo run); a release build is materially faster",
  };

  mkdirSync(path.join(ROOT, "marketing"), { recursive: true });
  const file = path.join(ROOT, "marketing", `load-tier-${tier}.json`);
  writeFileSync(file, `${JSON.stringify(report, null, 2)}\n`);
  process.stdout.write(`\n[marketing] wrote ${path.relative(ROOT, file)}\n`);
  process.exit(code ?? 1);
});
