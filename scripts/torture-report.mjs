#!/usr/bin/env node
/**
 * Run a torture scenario and record the result as a GitHub issue comment.
 *
 *   node scripts/torture-report.mjs --scenario worlds-1000          # dry run
 *   node scripts/torture-report.mjs --scenario worlds-1000 --post   # publish
 *
 * # Why the results go somewhere durable
 *
 * A load test whose output lives in a terminal is a load test nobody can cite.
 * The numbers scroll away, the next run overwrites the log, and six weeks later
 * the only record that the engine ever carried ten thousand subscribers is
 * somebody's memory of having seen it. Posting each run to a tracking issue
 * gives every scenario a dated history: what it measured, on what commit, and
 * whether it passed — which is also the only honest basis for quoting any of
 * it publicly.
 *
 * One issue per scenario, comments appended per run. Not one issue per run:
 * that buries the trend, and the trend is the interesting part.
 *
 * # Why only from `main`
 *
 * A number is worth recording when it describes the shared history of the
 * project. A run from a feature branch describes an experiment, and mixing the
 * two into one record makes both useless — you would no longer be able to read
 * a scenario's issue and know what the engine does today. So this refuses to
 * post from anywhere but `main`, and says so rather than posting quietly.
 *
 * # Why publishing is opt-in
 *
 * `--post` is required. Without it this prints exactly what it would send.
 * Creating issues in a shared repository is not something a script should do
 * as a side effect of somebody running a test to see what happens.
 */

import { execFile, spawn } from "node:child_process";
import { promisify } from "node:util";
import process from "node:process";

import { scenarioById } from "./torture-scenarios.mjs";

const exec = promisify(execFile);

const args = process.argv.slice(2);
const scenarioFlag = args.indexOf("--scenario");
if (scenarioFlag < 0) {
  console.error(
    "Usage: node scripts/torture-report.mjs --scenario <id> [--post]\n" +
      "       node scripts/torture.mjs --list   (to see scenario ids)",
  );
  process.exit(2);
}

const scenario = scenarioById(args[scenarioFlag + 1]);
const shouldPost = args.includes("--post");
const ISSUE_LABEL = "torture";

/** Current branch, or null if this is not a git working tree. */
async function currentBranch() {
  try {
    const { stdout } = await exec("git", ["rev-parse", "--abbrev-ref", "HEAD"]);
    return stdout.trim();
  } catch {
    return null;
  }
}

async function headCommit() {
  try {
    const { stdout } = await exec("git", ["rev-parse", "--short", "HEAD"]);
    return stdout.trim();
  } catch {
    return "unknown";
  }
}

/** Run the scenario, echoing as it goes, and keep the output for parsing. */
function runScenario() {
  return new Promise((resolve) => {
    const child = spawn(
      "node",
      ["scripts/torture.mjs", "--scenario", scenario.id],
      {
        env: { ...process.env, CONFIRM: "1" },
        stdio: ["ignore", "pipe", "pipe"],
      },
    );

    const chunks = [];
    const collect = (stream, sink) => {
      stream.on("data", (chunk) => {
        chunks.push(String(chunk));
        sink.write(chunk);
      });
    };
    collect(child.stdout, process.stdout);
    collect(child.stderr, process.stderr);

    child.on("exit", (code) =>
      resolve({ code: code ?? 1, output: chunks.join("") }),
    );
  });
}

/**
 * The `[torture] key=value ...` summary lines, which are the same lines the
 * assertions are made against rather than a second measurement for display.
 */
function parseSummaries(output) {
  const rows = [];
  for (const raw of output.split("\n")) {
    const line = raw.replace(/\[[0-9;]*m/g, "");
    const match = /\[torture\]\s+(.*)$/.exec(line);
    if (!match) continue;
    const body = match[1].trim();
    if (!/^[a-zA-Z]+=/.test(body)) continue;
    // Run bookkeeping, not measurement: the project name and ports are
    // ephemeral, and the trailing verdict is already the heading. A record
    // that mixes them in reads as though they were results.
    if (/^tier=\d+\s+project=/.test(body)) continue;
    if (/^tier=\d+(\s+(passed|FAILED))?$/.test(body)) continue;
    rows.push(body);
  }
  return rows;
}

/** Peak container usage, if the run's own metrics line reported any. */
function parsePubSubPeaks(output) {
  const peaks = {};
  for (const key of ["sockets", "subs_open", "subs_lagged", "subs_refused"]) {
    const values = [...output.matchAll(new RegExp(`${key}=(\\d+)`, "g"))].map(
      (m) => Number(m[1]),
    );
    if (values.length > 0) peaks[key] = Math.max(...values);
  }
  const pollDeltas = [...output.matchAll(/polls=\d+ \(\+(\d+)\)/g)].map((m) =>
    Number(m[1]),
  );
  // The first interval is partial and the last may be too, so the useful figure
  // is the slowest full interval, not the slowest reading.
  const interior = pollDeltas.slice(1, -1);
  if (interior.length > 0) peaks.min_polls_per_10s = Math.min(...interior);
  peaks.stall_alarms = (output.match(/NO POLLS COMPLETED/g) ?? []).length;
  return peaks;
}

function buildComment({ passed, summaries, peaks, commit, started, seconds }) {
  const verdict = passed ? "**PASSED**" : "**FAILED**";
  const lines = [
    `## ${scenario.title} — ${verdict}`,
    "",
    `> ${scenario.question}`,
    "",
    `| | |`,
    `|---|---|`,
    `| Scenario | \`${scenario.id}\` (tier ${scenario.tier}) |`,
    `| Commit | \`${commit}\` |`,
    `| Started | ${started} |`,
    `| Duration | ${seconds}s |`,
    "",
  ];

  if (summaries.length > 0) {
    lines.push("### What the run measured", "", "```");
    for (const row of summaries) lines.push(row);
    lines.push("```", "");
  }

  const peakKeys = Object.keys(peaks);
  if (peakKeys.length > 0) {
    lines.push("### Backplane health", "", "```");
    for (const key of peakKeys) lines.push(`${key}=${peaks[key]}`);
    lines.push("```", "");
    if (peaks.stall_alarms > 0) {
      lines.push(
        `> ⚠️ The delivery loop reported **${peaks.stall_alarms}** interval(s) with no completed polls. ` +
          `Events stay durable, but nothing reached clients live during them.`,
        "",
      );
    }
  }

  lines.push(
    passed ? "### What this establishes" : "### What a failure here means",
    "",
    passed ? scenario.proves : scenario.failureMeans,
    "",
    "<sub>Posted by `scripts/torture-report.mjs`. Every figure is parsed from the run that just happened; none is transcribed.</sub>",
  );

  return lines.join("\n");
}

/** Find this scenario's tracking issue, or create it. */
async function trackingIssue(title) {
  const { stdout } = await exec("gh", [
    "issue",
    "list",
    "--state",
    "all",
    "--search",
    title,
    "--json",
    "number,title",
    "--limit",
    "50",
  ]);
  const existing = JSON.parse(stdout).find((i) => i.title === title);
  if (existing) return existing.number;

  const body = [
    `Rolling record of the \`${scenario.id}\` torture scenario.`,
    "",
    `**Question:** ${scenario.question}`,
    "",
    `**A pass establishes:** ${scenario.proves}`,
    "",
    `**A failure means:** ${scenario.failureMeans}`,
    "",
    `Run it with \`node scripts/torture.mjs --scenario ${scenario.id}\`. ` +
      `See \`docs/torture-tests.md\` for why these exist.`,
  ].join("\n");

  const created = await exec("gh", [
    "issue",
    "create",
    "--title",
    title,
    "--body",
    body,
    ...(await labelArgs()),
  ]);
  const number = /\/issues\/(\d+)/.exec(created.stdout)?.[1];
  if (!number)
    throw new Error(`Could not read issue number: ${created.stdout}`);
  return Number(number);
}

/** Use the label only if it exists; a missing label should not fail a run. */
async function labelArgs() {
  try {
    const { stdout } = await exec("gh", [
      "label",
      "list",
      "--json",
      "name",
      "--limit",
      "200",
    ]);
    const has = JSON.parse(stdout).some((l) => l.name === ISSUE_LABEL);
    return has ? ["--label", ISSUE_LABEL] : [];
  } catch {
    return [];
  }
}

const started = new Date();
const branch = await currentBranch();
const commit = await headCommit();

console.log(
  `[report] scenario=${scenario.id} branch=${branch} commit=${commit}`,
);

const { code, output } = await runScenario();
const seconds = Math.round((Date.now() - started.getTime()) / 1000);
const passed = code === 0;

const comment = buildComment({
  passed,
  summaries: parseSummaries(output),
  peaks: parsePubSubPeaks(output),
  commit,
  started: started.toISOString(),
  seconds,
});

console.log("\n" + "─".repeat(72));
console.log(comment);
console.log("─".repeat(72) + "\n");

if (!shouldPost) {
  console.log(
    "[report] dry run — nothing was posted. Re-run with --post to publish.",
  );
  process.exit(passed ? 0 : 1);
}

if (branch !== "main") {
  console.error(
    `[report] refusing to post from branch "${branch}".\n` +
      "         A scenario's issue is the record of what the engine does on main;\n" +
      "         mixing branch experiments into it makes both unreadable.",
  );
  process.exit(passed ? 0 : 1);
}

const title = `Torture: ${scenario.title} (${scenario.id})`;
const issue = await trackingIssue(title);
await exec("gh", ["issue", "comment", String(issue), "--body", comment]);
console.log(`[report] posted to issue #${issue} — ${title}`);

process.exit(passed ? 0 : 1);
