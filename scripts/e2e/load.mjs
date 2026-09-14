/**
 * Was the machine busy while the suite ran?
 *
 * On 2026-09-14 a run had six failures — lighting, live-sync reconnect, a peer
 * partition, a grid default, a genie item — and every one passed in a quiet
 * re-run. The cause was a cold Rust build in another worktree, running beside
 * the suite. Nothing in the harness output hinted at it, so the failures read
 * as six product bugs.
 *
 * This samples the load average and looks for heavy processes the harness did
 * not start: compilers (`rustc`, `cargo`, `clippy-driver`, `wasm-opt`) and
 * other e2e runs. A process is the harness's own when it descends from this
 * pid, which is what separates the build this run does from a build someone
 * else started. The verdict and the evidence go into the summary, so a
 * timeout-heavy failure list arrives with "the machine was busy" beside it.
 */

import { spawnSync } from "node:child_process";
import { readlinkSync } from "node:fs";
import { availableParallelism, freemem, loadavg, totalmem } from "node:os";

const COMPILERS = new Set([
  "rustc",
  "cargo",
  "clippy-driver",
  "wasm-opt",
  "wasm-bindgen",
]);

function processTable() {
  const result = spawnSync("ps", ["-eo", "pid=,ppid=,pcpu=,comm=,args="], {
    encoding: "utf-8",
  });
  if (result.status !== 0) return [];
  return result.stdout
    .split("\n")
    .map((line) => /^\s*(\d+)\s+(\d+)\s+([\d.]+)\s+(\S+)\s+(.*)$/.exec(line))
    .filter(Boolean)
    .map(([, pid, ppid, pcpu, comm, args]) => ({
      pid: Number(pid),
      ppid: Number(ppid),
      pcpu: Number(pcpu),
      comm,
      args,
    }));
}

/** Every pid descending from `root`, `root` included. */
function descendants(table, root) {
  const children = new Map();
  for (const p of table) {
    if (!children.has(p.ppid)) children.set(p.ppid, []);
    children.get(p.ppid).push(p.pid);
  }
  const seen = new Set([root]);
  const stack = [root];
  while (stack.length) {
    for (const child of children.get(stack.pop()) ?? []) {
      if (!seen.has(child)) {
        seen.add(child);
        stack.push(child);
      }
    }
  }
  return seen;
}

function heavyKind(p) {
  if (COMPILERS.has(p.comm)) return p.comm;
  if (/e2e-parallel\.mjs/.test(p.args) && p.comm === "node") return "e2e run";
  return null;
}

function cwdOf(pid) {
  try {
    return readlinkSync(`/proc/${pid}/cwd`);
  } catch {
    return "?";
  }
}

export function startLoadMonitor({ intervalMs = 30_000 } = {}) {
  const cpus = availableParallelism();
  const samples = [];
  /** `kind@cwd` → what was seen of it. */
  const foreign = new Map();
  let phase = "setup";

  const sample = () => {
    const [load1, load5, load15] = loadavg();
    const table = processTable();
    const ours = descendants(table, process.pid);
    const seenNow = new Set();
    for (const p of table) {
      const kind = heavyKind(p);
      if (!kind || ours.has(p.pid)) continue;
      const cwd = cwdOf(p.pid);
      const key = `${kind}@${cwd}`;
      const entry = foreign.get(key) ?? {
        kind,
        cwd,
        samples: 0,
        phases: new Set(),
        maxProcesses: 0,
        pids: new Set(),
        example: p.args.slice(0, 160),
      };
      if (!seenNow.has(key)) {
        entry.samples += 1;
        entry.currentProcesses = 0;
      }
      seenNow.add(key);
      entry.currentProcesses += 1;
      entry.maxProcesses = Math.max(entry.maxProcesses, entry.currentProcesses);
      entry.phases.add(phase);
      entry.pids.add(p.pid);
      foreign.set(key, entry);
    }
    samples.push({
      at: new Date().toISOString(),
      phase,
      load1: Number(load1.toFixed(2)),
      load5: Number(load5.toFixed(2)),
      load15: Number(load15.toFixed(2)),
      freeMemGiB: Number((freemem() / 2 ** 30).toFixed(1)),
      foreign: [...seenNow],
    });
  };

  sample();
  const timer = setInterval(sample, intervalMs);
  timer.unref();

  return {
    setPhase(next) {
      phase = next;
      sample();
    },
    /** The summary so far, without stopping. */
    snapshot() {
      return summarise({ cpus, samples, foreign, intervalMs });
    },
    stop() {
      clearInterval(timer);
      sample();
      return summarise({ cpus, samples, foreign, intervalMs });
    },
  };
}

function summarise({ cpus, samples, foreign, intervalMs }) {
  const tests = samples.filter((s) => s.phase === "tests");
  const window = tests.length ? tests : samples;
  const loads = window.map((s) => s.load1);
  const max = Math.max(...loads);
  const mean = loads.reduce((a, b) => a + b, 0) / loads.length;
  const overCpus = window.filter((s) => s.load1 > cpus).length;

  const foreignList = [...foreign.values()].map((f) => ({
    kind: f.kind,
    cwd: f.cwd,
    samples: f.samples,
    duringTests: f.phases.has("tests"),
    maxProcesses: f.maxProcesses,
    example: f.example,
  }));
  const foreignDuringTests = foreignList.filter((f) => f.duringTests);

  const notes = [];
  if (max > cpus) {
    notes.push(
      `load average peaked at ${max} on ${cpus} CPUs (mean ${mean.toFixed(1)}; ` +
        `${overCpus} of ${window.length} samples above the CPU count)`,
    );
  }
  for (const f of foreignDuringTests) {
    notes.push(
      `${f.kind} not started by this run was active in ${f.cwd} ` +
        `(${f.samples} sample${f.samples === 1 ? "" : "s"}, up to ${f.maxProcesses} process${
          f.maxProcesses === 1 ? "" : "es"
        })`,
    );
  }
  const busy =
    mean > cpus * 0.9 ||
    overCpus >= Math.max(2, window.length * 0.25) ||
    foreignDuringTests.some((f) => f.samples >= 2 || f.kind === "e2e run");

  return {
    busy,
    notes,
    cpus,
    totalMemGiB: Number((totalmem() / 2 ** 30).toFixed(1)),
    intervalSeconds: intervalMs / 1000,
    atStart: samples[0],
    duringTests: {
      samples: tests.length,
      load1Max: tests.length ? Math.max(...tests.map((s) => s.load1)) : null,
      load1Mean: tests.length
        ? Number(
            (tests.reduce((a, s) => a + s.load1, 0) / tests.length).toFixed(2),
          )
        : null,
      minFreeMemGiB: tests.length
        ? Math.min(...tests.map((s) => s.freeMemGiB))
        : null,
    },
    foreign: foreignList,
    samples,
  };
}
