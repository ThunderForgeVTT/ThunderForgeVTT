/**
 * The slice listing: what each slice runs and what it costs (spec 060, US4).
 *
 * `scripts/e2e-slice.mjs list` reads the files and hands them here; this
 * module is pure, so the state rules — the 600 s target, the 720 s limit, red
 * beating both — are unit-tested against plain objects.
 *
 * A slice's time is either *measured* (a record the runner wrote after a real
 * `--slice --record-durations` run, stack start and engine build included) or
 * an *estimate* (the sum of its specs' recorded per-spec times, which leaves
 * both of those out). The two are never shown the same way: an estimate is
 * always printed with `est.` in front of it (FR-016), because a number that
 * looks measured and is not is exactly how "fits in ten minutes" becomes a
 * belief instead of a fact.
 */

import { ownershipOf, resolveSlice, sliceLanes, specKey } from "./slices.mjs";

/** SC-002's target and limit for one slice's wall time, in seconds. */
export const TARGET_SECONDS = 600;
export const LIMIT_SECONDS = 720;

/** The states `list` shows, per data-model.md "Slice measurement". */
export const STATES = [
  "not measured",
  "measured",
  "over target",
  "over limit",
  "red",
];

/**
 * The state of a slice from its measurement record (or `undefined`).
 *
 * Red wins over any time: a slice that failed has not proven anything, and a
 * fast red run is not "measured" in any sense worth printing in green.
 */
export function sliceState(record) {
  if (!record) return "not measured";
  if ((record.failed ?? 0) > 0) return "red";
  if (record.wallSeconds > LIMIT_SECONDS) return "over limit";
  if (record.wallSeconds > TARGET_SECONDS) return "over target";
  return "measured";
}

/**
 * The per-spec estimate for these spec files: the sum of what `durations`
 * (the runner's `.e2e-shards-durations*.json`, keyed `e2e/x.spec.ts`) holds,
 * and which files it has nothing for. A missing file adds nothing, so an
 * estimate with gaps is low — the listing marks it.
 */
export function estimateSeconds(specFiles, durations) {
  let seconds = 0;
  const missing = [];
  for (const file of specFiles) {
    const value = durations[file] ?? durations[`e2e/${specKey(file)}`];
    if (typeof value === "number") seconds += value;
    else missing.push(file);
  }
  return { seconds, missing };
}

/**
 * One row per slice, in list order.
 *
 * `measurements` is `readSliceDurations()`'s object (keyed by slice name),
 * `durations` the per-spec times. `specFiles` defaults to the real tree
 * through `resolveSlice`, so tests pass their own.
 */
export function buildRows(slices, { specFiles, measurements, durations }) {
  return slices.map((slice) => {
    const resolved = resolveSlice(slice.name, slices, specFiles);
    const owned = specFiles.filter(
      (file) => ownershipOf(file, slices)?.slice === slice.name,
    );
    const record = measurements[slice.name];
    const estimate = estimateSeconds(resolved, durations);
    return {
      name: slice.name,
      specs: resolved.length,
      own: owned.length,
      neighbours: slice.neighbours.length,
      lanes: sliceLanes(resolved),
      standalone: slice.standalone ?? null,
      state: sliceState(record),
      measured: record
        ? {
            wallSeconds: record.wallSeconds,
            measuredAt: record.measuredAt,
            commit: record.commit,
            passed: record.passed,
            failed: record.failed,
            flaky: record.flaky,
            skipped: record.skipped,
            specs: record.specs,
          }
        : null,
      estimate: {
        seconds: Math.round(estimate.seconds),
        missing: estimate.missing,
      },
    };
  });
}

/** `6m 40s` for a measured time. */
export function formatMeasured(seconds) {
  const whole = Math.round(seconds);
  return `${Math.floor(whole / 60)}m ${String(whole % 60).padStart(2, "0")}s`;
}

/**
 * `est. 6.3m` for an estimate, with a `*` when some specs have no recorded
 * time (so the true figure is higher). Never without the `est.`.
 */
export function formatEstimate(estimate) {
  const minutes = (estimate.seconds / 60).toFixed(1);
  return `est. ${minutes}m${estimate.missing.length > 0 ? "*" : ""}`;
}

/** The time column: measured if there is a record, else the labelled estimate. */
export function timeCell(row) {
  return row.measured
    ? formatMeasured(row.measured.wallSeconds)
    : formatEstimate(row.estimate);
}

/** The lanes column, with `+standalone` where a stack-free half exists. */
export function lanesCell(row) {
  const lanes = row.lanes.join(",");
  return row.standalone ? `${lanes} +standalone` : lanes;
}

function measuredCell(row) {
  if (!row.measured) return "not measured";
  const when = `${row.measured.measuredAt} ${row.measured.commit}`;
  return row.state === "measured" ? when : `${when}  ${row.state}`;
}

/** Counts for the footer: slices, spec runs, and each state that matters. */
export function totals(rows, uniqueSpecs) {
  const count = (state) => rows.filter((row) => row.state === state).length;
  return {
    slices: rows.length,
    specRuns: rows.reduce((sum, row) => sum + row.specs, 0),
    specs: uniqueSpecs,
    measured: rows.filter((row) => row.measured).length,
    overTarget: count("over target"),
    overLimit: count("over limit"),
    red: count("red"),
  };
}

/** The table `pnpm e2e:slices` prints, with its footer. */
export function formatTable(rows, sums) {
  const header = ["slice", "specs", "own", "nb", "lanes", "time", "measured"];
  const cells = rows.map((row) => [
    row.name,
    String(row.specs),
    String(row.own),
    String(row.neighbours),
    lanesCell(row),
    timeCell(row),
    measuredCell(row),
  ]);
  const widths = header.map((title, column) =>
    Math.max(title.length, ...cells.map((line) => line[column].length)),
  );
  // Counts are right-aligned so their digits line up; text is left-aligned.
  const numeric = new Set([1, 2, 3]);
  const render = (line) =>
    line
      .map((cell, column) =>
        column === line.length - 1
          ? cell
          : numeric.has(column)
            ? cell.padStart(widths[column])
            : cell.padEnd(widths[column]),
      )
      .join("  ")
      .trimEnd();
  const lines = [render(header), ...cells.map(render), ""];
  lines.push(
    `${sums.slices} slices, ${sums.specs} specs (${sums.specRuns} runs counting neighbours); ` +
      `${sums.measured} measured, ${sums.overTarget} over target, ${sums.overLimit} over limit, ${sums.red} red`,
  );
  if (rows.some((row) => !row.measured)) {
    lines.push(
      "est. = the sum of recorded per-spec times, without stack start or engine build; * = some specs have no recorded time.",
    );
  }
  return lines.join("\n");
}

/**
 * `list <name>`: everything about one slice — what it owns and how, its
 * neighbours with their seams, its `paths`, its standalone half and how to
 * run it.
 */
export function formatDetail(slice, row, { slices, specFiles }) {
  const lines = [`${slice.name} — ${slice.summary}`, ""];
  if (row.measured) {
    const m = row.measured;
    lines.push(
      `Time: ${formatMeasured(m.wallSeconds)} (${row.state}), measured ${m.measuredAt} at ${m.commit}: ` +
        `${m.passed} passed, ${m.failed} failed, ${m.flaky} flaky, ${m.skipped} skipped`,
    );
  } else {
    lines.push(`Time: ${formatEstimate(row.estimate)} (not measured)`);
    if (row.estimate.missing.length > 0) {
      lines.push(
        `      no recorded time for: ${row.estimate.missing.map(specKey).join(", ")}`,
      );
    }
  }
  lines.push(`Lanes: ${row.lanes.join(", ")}`, "");

  const owned = specFiles
    .map((file) => ({ file, ownership: ownershipOf(file, slices) }))
    .filter(({ ownership }) => ownership?.slice === slice.name);
  lines.push(`Owns (${owned.length}):`);
  const width = Math.max(0, ...owned.map(({ file }) => specKey(file).length));
  for (const { file, ownership } of owned) {
    const how = ownership.exact ? "exact" : `prefix "${ownership.entry}"`;
    lines.push(`  ${specKey(file).padEnd(width)}  ${how}`);
  }
  lines.push("", `Neighbours (${slice.neighbours.length}):`);
  for (const { spec, seam } of slice.neighbours) {
    lines.push(`  ${spec} — ${seam}`);
  }
  lines.push("", `Paths (${slice.paths.length}):`);
  for (const glob of slice.paths) lines.push(`  ${glob}`);
  lines.push("");
  if (slice.standalone) lines.push(`Standalone: ${slice.standalone}`);
  lines.push(`Run: pnpm e2e:${slice.name}`);
  lines.push(
    `     pnpm e2e:${slice.name}:integration -- --record-durations  (to measure it)`,
  );
  return lines.join("\n");
}
