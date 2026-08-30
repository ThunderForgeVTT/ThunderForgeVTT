#!/usr/bin/env node
/**
 * One command for "is this repository tidy" — formatting and lint, both
 * languages, from anywhere.
 *
 * # Why this exists
 *
 * The four checks below live in three directories and two toolchains, and
 * before this they were four separate invocations nobody ran in the same
 * sitting. The root `format` script came closest and could not fail: it
 * joined its steps with `;`, so a failing prettier was followed by a `cargo
 * fmt` that rewrote files and returned zero, and the whole thing reported
 * success. A gate that cannot fail is not a gate.
 *
 * Every step runs even when an earlier one fails, and the summary at the end
 * lists all of them. Stopping at the first failure means finding out about
 * the next one only after fixing this one, which is how a tidy-up turns into
 * an afternoon.
 *
 * # `--fix`
 *
 * Without it, nothing is written and the exit code answers the question.
 * With it, the two formatters and eslint rewrite what they can, and the
 * summary says what was changed. Kept behind a flag because a check that
 * silently edits the working tree is a check you stop trusting.
 *
 * # Scope
 *
 * Deliberately *not* the test suites. Those take minutes, need a database
 * and a dev server, and answer a different question. This one is meant to be
 * cheap enough to run before every commit.
 */

import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const fix = process.argv.includes("--fix");

/**
 * One check. `cwd` is relative to the repo root so this works from anywhere,
 * which matters: `cargo test` from `apps/web` cannot find the root `.env`,
 * and the same class of mistake is easy to make here.
 */
const steps = [
  {
    name: "rust format",
    cwd: ".",
    command: fix ? ["cargo", "fmt", "--all"] : ["cargo", "fmt", "--all", "--check"],
  },
  {
    name: "rust lint",
    cwd: ".",
    // `--all-targets` includes tests and benches, which is where lint debt
    // hides — code nobody reads until it fails to compile. The engine crate
    // is excluded for the same reason the test command excludes it: bevy is
    // taken with no windowing backend, so it cannot build for the host.
    command: [
      "cargo",
      "clippy",
      "--workspace",
      "--exclude",
      "thunderforge_engine",
      "--all-targets",
    ],
  },
  {
    name: "web format",
    cwd: "apps/web",
    // The project's own glob, from `apps/web`'s `format` script. It excludes
    // e2e; widening it is a decision to make deliberately, not inside a
    // convenience wrapper.
    command: [
      "pnpm",
      "exec",
      "prettier",
      fix ? "--write" : "--check",
      "{index.html,vite.config.mts,eslint.config.mjs,.prettierrc.json,src/**/*.{ts,tsx,scss},scripts/**/*.mjs}",
    ],
  },
  {
    name: "web lint",
    cwd: "apps/web",
    command: fix
      ? ["pnpm", "exec", "eslint", ".", "--ext", ".ts,.tsx", "--fix"]
      : ["pnpm", "exec", "eslint", ".", "--ext", ".ts,.tsx", "--max-warnings=0"],
  },
  {
    // Spec 029 T064. The TypeScript SDK types are generated from the Rust
    // types by ts-rs, and a generated file that has drifted from its source
    // is worse than no generated file: the compiler goes on cheerfully
    // checking callers against a contract the engine no longer speaks, which
    // is precisely the silent-drift failure the typed SDK exists to retire.
    //
    // `--fix` regenerates; a plain run only reports, so the gate cannot
    // quietly rewrite the tree it is meant to be checking.
    name: "sdk bindings",
    cwd: ".",
    command: fix
      ? ["pnpm", "run", "sdk:bindings"]
      : ["pnpm", "run", "sdk:check"],
  },
  {
    // Spec 030 T076. FR-039 says the interaction plugin owns no effect, and
    // that is a claim a behavioural test cannot catch eroding until something
    // has already broken. This catches it in the diff.
    //
    // Not affected by `--fix`: there is nothing mechanical to rewrite. A
    // violation is either an effect that belongs in a contributor or a word in
    // a comment, and both need a person.
    name: "interaction seam",
    cwd: ".",
    command: ["node", "./scripts/check-interaction-seam.mjs"],
  },
];

const results = [];
for (const step of steps) {
  process.stdout.write(`\n── ${step.name}${fix ? " (fixing)" : ""}\n`);
  const [command, ...args] = step.command;
  const run = spawnSync(command, args, {
    cwd: path.join(repoRoot, step.cwd),
    stdio: "inherit",
  });
  // A step that could not be started at all is a failure, not a pass — a
  // missing toolchain must never read as "nothing to report".
  const ok = run.error === undefined && run.status === 0;
  results.push({ name: step.name, ok, detail: run.error?.message });
}

process.stdout.write("\n── summary\n");
for (const result of results) {
  process.stdout.write(
    `${result.ok ? "  ok  " : "  FAIL"}  ${result.name}${
      result.detail ? ` (${result.detail})` : ""
    }\n`,
  );
}

const failed = results.filter((result) => !result.ok);
if (failed.length > 0) {
  process.stdout.write(
    `\n${failed.length} of ${results.length} checks failed.${
      fix ? "" : " Re-run with --fix to rewrite what can be rewritten."
    }\n`,
  );
  process.exit(1);
}
process.stdout.write(`\nAll ${results.length} checks passed.\n`);
