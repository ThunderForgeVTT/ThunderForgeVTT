/**
 * What a run found, in one file and one short digest.
 *
 * Playwright already writes a JSON report per lane, but reading a run meant
 * opening one per shard and scrolling a log in which four shards interleave.
 * This reads those reports back and writes `e2e-summary.json` (gitignored):
 * per lane, the counts; per failure, the spec, the title, the first line of
 * the error and where its screenshot, trace and error context are; and the
 * load notes from `load.mjs`, so a failure list and "the machine was busy"
 * arrive together.
 *
 * It also appends each run's failures and flakes to `.e2e-history.jsonl`
 * (gitignored), which `scripts/e2e-flakes.mjs` reads to name the tests that
 * fail in more than one recent run — the difference between a flake and a
 * one-off is not visible from inside a single run.
 */

import { execFileSync } from "node:child_process";
import { appendFileSync, readFileSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";

export const SUMMARY_NAME = "e2e-summary.json";
export const HISTORY_NAME = ".e2e-history.jsonl";

// eslint-disable-next-line no-control-regex
const ANSI = /\x1b\[[0-9;]*m/g;

/** The first meaningful line of a Playwright error. */
function firstErrorLine(errors) {
  const messages = errors
    .map((e) => String(e?.message ?? e?.value ?? "").replace(ANSI, ""))
    .filter(Boolean);
  // The load-diagnostics fixture adds this in teardown, after the assertion
  // that failed because of it; it is the more useful of the two.
  const preferred =
    messages.find((m) => m.startsWith("Error: frontend failed to load")) ??
    messages[0];
  if (!preferred) return null;
  return (
    preferred
      .split("\n")
      .map((l) => l.trim())
      .find(Boolean)
      ?.slice(0, 400) ?? null
  );
}

const TIMEOUT = /timeout|timed out|exceeded/i;

/** Read one lane's JSON report into counts, failures and flakes. */
export function readLane(result, root) {
  const lane = {
    label: result.label,
    shard: result.index,
    verdict: result.failed ? "failed" : result.skipped ? "no files" : "passed",
    reasons: result.reasons ?? [],
    report: result.reportPath ? relative(root, result.reportPath) : null,
    counts: null,
    durationSeconds: null,
    failures: [],
    flaky: [],
  };
  if (!result.reportPath) return lane;

  let report;
  try {
    report = JSON.parse(readFileSync(result.reportPath, "utf-8"));
  } catch {
    return lane;
  }
  const stats = report.stats ?? {};
  lane.counts = {
    passed: stats.expected ?? 0,
    failed: stats.unexpected ?? 0,
    flaky: stats.flaky ?? 0,
    skipped: stats.skipped ?? 0,
  };
  lane.durationSeconds = Math.round((stats.duration ?? 0) / 1000);

  const walk = (suite, trail) => {
    for (const spec of suite.specs ?? []) {
      for (const test of spec.tests ?? []) {
        if (test.status !== "unexpected" && test.status !== "flaky") continue;
        const entry = {
          spec: `e2e/${spec.file ?? suite.file}`,
          line: spec.line ?? null,
          title: [...trail, spec.title].join(" › "),
        };
        if (test.status === "flaky") {
          lane.flaky.push(entry);
          continue;
        }
        const last = test.results?.[test.results.length - 1] ?? {};
        const attachment = (name) =>
          (last.attachments ?? []).find((a) => a.name === name && a.path)
            ?.path ?? null;
        const error = firstErrorLine(
          last.errors?.length ? last.errors : [last.error].filter(Boolean),
        );
        lane.failures.push({
          ...entry,
          status: last.status ?? "failed",
          error,
          timeout: last.status === "timedOut" || TIMEOUT.test(error ?? ""),
          screenshot: attachment("screenshot"),
          trace: attachment("trace"),
          errorContext: attachment("error-context"),
          // Inline in the report (base64), not a file: the fixture attaches a
          // body. Kept short; the whole thing is in the lane's JSON report.
          loadDiagnostics: (() => {
            const body = (last.attachments ?? []).find(
              (a) => a.name === "load-diagnostics" && a.body,
            )?.body;
            return body
              ? Buffer.from(body, "base64").toString("utf-8").slice(0, 2000)
              : null;
          })(),
        });
      }
    }
    for (const child of suite.suites ?? []) {
      walk(child, [...trail, child.title]);
    }
  };
  // The top-level suite's title is the file name, which `spec` already says.
  for (const suite of report.suites ?? []) walk(suite, []);

  for (const error of report.errors ?? []) {
    lane.failures.push({
      spec: error.location?.file
        ? relative(join(root, "apps/web"), error.location.file)
        : null,
      line: error.location?.line ?? null,
      title: "(runner error: the spec did not load or the run aborted)",
      status: "error",
      error: firstErrorLine([error]),
      timeout: false,
      screenshot: null,
      trace: null,
      errorContext: null,
      loadDiagnostics: null,
    });
  }
  return lane;
}

function gitHead(root) {
  try {
    return execFileSync("git", ["rev-parse", "--short", "HEAD"], {
      cwd: root,
      encoding: "utf-8",
      stdio: ["ignore", "pipe", "ignore"],
    }).trim();
  } catch {
    return null;
  }
}

/** Write `e2e-summary.json` and append to the history. Returns the summary. */
export function writeRunSummary({ root, results, load, args, startedAt }) {
  const lanes = results.map((result) => readLane(result, root));
  const failures = lanes.flatMap((lane) =>
    lane.failures.map((f) => ({ lane: lane.label, shard: lane.shard, ...f })),
  );
  const flaky = lanes.flatMap((lane) =>
    lane.flaky.map((f) => ({ lane: lane.label, shard: lane.shard, ...f })),
  );
  const totals = { passed: 0, failed: 0, flaky: 0, skipped: 0 };
  for (const lane of lanes) {
    for (const key of Object.keys(totals)) {
      totals[key] += lane.counts?.[key] ?? 0;
    }
  }
  const timeouts = failures.filter((f) => f.timeout).length;

  const summary = {
    startedAt,
    finishedAt: new Date().toISOString(),
    commit: gitHead(root),
    args,
    verdict: lanes.some((l) => l.verdict === "failed") ? "failed" : "passed",
    totals,
    lanes: lanes.map(({ failures: _f, flaky: _k, ...lane }) => lane),
    failures,
    flaky,
    load: load
      ? {
          busy: load.busy,
          notes: load.notes,
          timeoutFailures: timeouts,
          ...load,
        }
      : null,
  };
  writeFileSync(
    join(root, SUMMARY_NAME),
    `${JSON.stringify(summary, null, 2)}\n`,
  );

  appendFileSync(
    join(root, HISTORY_NAME),
    `${JSON.stringify({
      startedAt,
      finishedAt: summary.finishedAt,
      commit: summary.commit,
      args,
      busy: load?.busy ?? null,
      totals,
      laneFailures: lanes
        .filter((l) => l.verdict === "failed" && l.failures.length === 0)
        .map((l) => ({ lane: l.label, reasons: l.reasons })),
      failures: failures.map(({ spec, title, lane, timeout }) => ({
        spec,
        title,
        lane,
        timeout,
      })),
      flaky: flaky.map(({ spec, title, lane }) => ({ spec, title, lane })),
    })}\n`,
  );
  return summary;
}

/** The lines to print at the very end of the log. */
export function digestLines(summary, root) {
  const lines = [];
  const { totals, failures, flaky, load } = summary;
  lines.push(
    `Totals: ${totals.passed} passed, ${totals.failed} failed, ${totals.flaky} flaky, ${totals.skipped} skipped.`,
  );
  if (failures.length) {
    lines.push(`Failure digest (${failures.length}):`);
    for (const f of failures) {
      lines.push(
        `  - [${f.lane} ${f.shard}] ${f.spec ?? "?"}${f.line ? `:${f.line}` : ""} › ${f.title}`,
      );
      if (f.error) lines.push(`      ${f.error}`);
      for (const key of ["screenshot", "errorContext", "trace"]) {
        if (f[key]) lines.push(`      ${key}: ${relative(root, f[key])}`);
      }
    }
  }
  for (const lane of summary.lanes) {
    if (lane.verdict === "failed" && !lane.counts) {
      lines.push(
        `  - [${lane.label} ${lane.shard}] ${lane.reasons.join(", ")}`,
      );
    }
  }
  if (flaky.length) {
    lines.push(`Flaky, passed on retry (${flaky.length}):`);
    for (const f of flaky) lines.push(`  - ${f.spec} › ${f.title}`);
  }
  if (load) {
    if (load.busy) {
      const timeouts = failures.filter((f) => f.timeout).length;
      lines.push(
        `The machine was busy during this run${
          failures.length
            ? ` (${timeouts} of ${failures.length} failure(s) were timeouts); re-run the failures on a quiet machine before believing them`
            : ""
        }:`,
      );
    } else if (load.notes.length) {
      lines.push("Load notes:");
    }
    for (const note of load.notes) lines.push(`  - ${note}`);
  }
  lines.push(
    `Summary: ${SUMMARY_NAME}; history: ${HISTORY_NAME} (node scripts/e2e-flakes.mjs).`,
  );
  return lines;
}
