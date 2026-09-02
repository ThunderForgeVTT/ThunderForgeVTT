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
 * and a dev server, and answer a different question.
 *
 * # Cost, and what the hooks make of it
 *
 * This was written expecting to be cheap enough to run before every commit.
 * Measured, it is two things rather than one. rustfmt, prettier and the two
 * node checkers read files and exit — about 3.6s together, and that number
 * does not move. clippy and eslint compile, so they are sub-second on a warm
 * tree and minutes after a rebase or a change to a widely-included header.
 *
 * So `.hooks/pre-commit` runs the flat-cost four by id and `.hooks/pre-push`
 * runs all eight. The point of the split is not that the compiled checks
 * matter less; it is that a hook with an unbounded worst case teaches people
 * to pass `--no-verify`, and a gate that is routinely bypassed gates nothing.
 */

import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const fix = process.argv.includes("--fix");

/**
 * `--only=<id>,<id>` runs a subset, by the `id` on each step below.
 *
 * This exists so the git hooks in `.hooks/` can select from *this* list
 * rather than restating the commands. A hook holding its own copy of
 * `cargo fmt --all --check` is a second gate that drifts from the first, and
 * the drift is silent: both keep passing, they just stop checking the same
 * thing. The hooks name ids; the commands live here only.
 */
const onlyArg = process.argv.find((arg) => arg.startsWith("--only="));
const only = onlyArg ? onlyArg.slice("--only=".length).split(",").filter(Boolean) : null;

/**
 * One check. `cwd` is relative to the repo root so this works from anywhere,
 * which matters: `cargo test` from `apps/web` cannot find the root `.env`,
 * and the same class of mistake is easy to make here.
 */
const steps = [
  {
    id: "rust-fmt",
    name: "rust format",
    cwd: ".",
    command: fix ? ["cargo", "fmt", "--all"] : ["cargo", "fmt", "--all", "--check"],
  },
  {
    id: "rust-lint",
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
    // The engine is excluded above because bevy is taken with no windowing
    // backend and cannot build for the host — which left the crate with no
    // lint gate at all, and 11 warnings nobody had seen. It builds fine for
    // the target it actually ships to: `wasm-pack` produces
    // wasm32-unknown-unknown, so any environment that can build the engine
    // can lint it here. `--lib` rather than `--all-targets` because the unit
    // tests are host tests and are covered by the workspace run above.
    id: "engine-lint",
    name: "engine lint (wasm)",
    cwd: ".",
    command: [
      "cargo",
      "clippy",
      "-p",
      "thunderforge_engine",
      "--target",
      "wasm32-unknown-unknown",
      "--lib",
    ],
  },
  {
    id: "web-fmt",
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
    id: "web-lint",
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
    id: "sdk",
    name: "sdk bindings",
    cwd: ".",
    command: fix
      ? ["pnpm", "run", "sdk:bindings"]
      : ["pnpm", "run", "sdk:check"],
  },
  {
    // Spec 032 FR-029. A system pack declares what it contributes and
    // nothing lists it — a property a behavioural test cannot catch eroding,
    // because a hand-maintained registry passes every test it has right up
    // until the eighth system costs as much to add as the first.
    //
    // Not affected by `--fix`: a violation is shared code that has learned a
    // system's name, and moving that knowledge back into the pack needs a
    // person.
    id: "registry",
    name: "system registry",
    cwd: ".",
    command: ["node", "./scripts/check-system-registry.mjs"],
  },
  {
    // Spec 030 T076. FR-039 says the interaction plugin owns no effect, and
    // that is a claim a behavioural test cannot catch eroding until something
    // has already broken. This catches it in the diff.
    //
    // Not affected by `--fix`: there is nothing mechanical to rewrite. A
    // violation is either an effect that belongs in a contributor or a word in
    // a comment, and both need a person.
    id: "seam",
    name: "interaction seam",
    cwd: ".",
    command: ["node", "./scripts/check-interaction-seam.mjs"],
  },
];

// An unknown id is a failure, not an empty run: a hook whose step was renamed
// must say so rather than quietly gating on nothing.
if (only) {
  const known = new Set(steps.map((step) => step.id));
  const unknown = only.filter((id) => !known.has(id));
  if (unknown.length > 0) {
    process.stdout.write(
      `unknown --only id(s): ${unknown.join(", ")}\nknown: ${[...known].join(", ")}\n`,
    );
    process.exit(2);
  }
}

const selected = only ? steps.filter((step) => only.includes(step.id)) : steps;

const results = [];
for (const step of selected) {
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
