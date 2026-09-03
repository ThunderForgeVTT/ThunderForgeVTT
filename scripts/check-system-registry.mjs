#!/usr/bin/env node
/**
 * FR-029, written as a check anyone can run.
 *
 * Spec 032 says a game system pack's contribution is **discovered**, not
 * listed: each pack submits what it contributes, and shared server code
 * collects them without knowing a single system's name.
 *
 * That claim is easy to state and easy to erode, and the erosion is
 * comfortable rather than painful — a central list works perfectly right up
 * until the eighth system costs as much to add as the first. `systems.rs`
 * previously held seven `register_*_system` functions and an initialiser with
 * a `// In future phases: register_coc7e_system(...)` comment already waiting
 * to become the eighth. Nobody wrote that in a moment of carelessness; it is
 * just what happens when a list exists.
 *
 * A behavioural test cannot catch this. A hand-maintained registry passes
 * every test it has, because being up to date is exactly what it is for. This
 * catches it in the diff instead, which is the same argument
 * `check-interaction-seam.mjs` makes for the interaction seam.
 *
 * # What is allowed, and why
 *
 * `src/app/src/system_packs.rs` is exempt. It holds one `use <pack> as _;`
 * line per bundled pack, and those lines are load-bearing for a reason that
 * was measured rather than assumed: a statically linked Rust crate nothing
 * references is never linked, and its `inventory` submissions vanish with it.
 * A binary depending on a submitting crate without naming a symbol from it
 * collected an empty set in both debug and release.
 *
 * Those lines are permitted because they carry no information about the
 * system — not its data shapes, not its validators, not its rules — so unlike
 * a validator list there is nothing in them that can drift out of step with a
 * pack. Everything else in shared server code must stay ignorant.
 *
 * # Why identifiers rather than an import graph
 *
 * An import check would pass against a `match game_system_id { "dnd5e" => ...`
 * — no new dependency, just knowledge, which is the likeliest shape the
 * violation actually takes. The identifiers are cruder and catch it.
 *
 * # False positives are the point
 *
 * A doc comment mentioning a system by name trips this. Rewording costs a
 * minute; the alternative is a check with holes carved into it for
 * convenience, which is a check nobody trusts.
 */

import { readdirSync, readFileSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

/** Read the bundled system ids from the packs themselves, never a list here. */
function bundledSystemIds() {
  const systemsDir = path.join(repoRoot, "packs", "systems");
  return readdirSync(systemsDir).filter((entry) =>
    statSync(path.join(systemsDir, entry)).isDirectory(),
  );
}

/**
 * Shared server code. Pack crates are not shared code — a pack naming itself
 * is the whole point — and neither are tests, which have to name a system to
 * assert anything about it. Tests reach this codebase two ways: inline
 * `#[cfg(test)]` modules, stripped below, and sibling `*_tests.rs` files
 * wired with `#[path]`, excluded here.
 */
function sharedServerSources() {
  // Two roots since the crate split: `src/server` is the server as a library
  // and `src/app` is the binary that composes it with the packs. Both are
  // shared code, and the binary is *especially* worth scanning — it is the one
  // place that legitimately knows packs exist, which makes it the comfortable
  // place for knowledge that should not be there. Missing it would have left
  // the rule enforced on the larger half and unenforced on the tempting one.
  const roots = [
    path.join(repoRoot, "src", "server", "src"),
    path.join(repoRoot, "src", "app", "src"),
  ];
  const out = [];
  const walk = (dir) => {
    for (const entry of readdirSync(dir)) {
      const full = path.join(dir, entry);
      if (statSync(full).isDirectory()) {
        walk(full);
      } else if (entry.endsWith(".rs")) {
        out.push(full);
      }
    }
  };
  for (const root of roots) walk(root);
  return out.filter((file) => {
    const name = path.basename(file);
    return (
      // The linkage modules, exempt for the reason in their own headers: one
      // `use <pack> as _;` line each, carrying no information that can drift.
      // `system_packs.rs` links the application; `test_packs.rs` links this
      // library's test binary, which links nothing on its own.
      name !== "system_packs.rs" &&
      name !== "test_packs.rs" &&
      // Test-support fixtures have to name systems to build one.
      name !== "test_support.rs" &&
      // This crate keeps some test modules in sibling `*_tests.rs` files
      // wired with `#[path]` — `interaction_tests.rs`, `system_rules_tests.rs`.
      // They carry no `#[cfg(test)]` marker of their own, so the stripper
      // below cannot see them, but they are tests and a test asserting "a
      // Genie actor derives its Wish Points" has to name Genie. Same
      // exemption as an inline test module, different file layout.
      !name.endsWith("_tests.rs")
    );
  });
}

/**
 * Strip `#[cfg(test)]` modules and test files.
 *
 * A test asserting "a Genie actor derives its Wish Points" must name Genie.
 * The rule is about what the *product* knows, not what its tests do.
 */
function withoutTests(source) {
  const marker = "#[cfg(test)]";
  const at = source.indexOf(marker);
  return at === -1 ? source : source.slice(0, at);
}

/**
 * Known violations, dated, each with the task that retires it.
 *
 * Not a hole in the check — a hole would be widening the rule so these stop
 * being violations. Each *is* a violation: shared code that decides something
 * per game system, which is precisely what FR-029 forbids. They are listed
 * because they are behavioural and pre-date this feature, and a gate that
 * fails on day one is a gate somebody turns off.
 *
 * Adding to this list requires a task id. If the list ever grows without one,
 * the check has become the thing it was written to prevent.
 */
const KNOWN = new Map();
// Empty, as of 2026-09-03, and that is the point of the list rather than a
// gap in it.
//
// It held one entry for the whole of spec 032: `graphql.rs` branched on one
// system's name during world creation to insert that system's session row.
// Retiring it took the increment ADR-063 sized — the server became a library
// so a pack could depend on it, and Genie's six tables, eleven models and
// 2,763 lines of GraphQL moved into `packs/systems/genie/server`. The row is
// still written; the pack writes it, through a world-creation hook the server
// runs without knowing whose it is.

const ids = bundledSystemIds();
const failures = [];
const stale = new Set(KNOWN.keys());

for (const file of sharedServerSources()) {
  const source = withoutTests(readFileSync(file, "utf8"));
  const relative = path.relative(repoRoot, file);
  source.split("\n").forEach((line, index) => {
    for (const id of ids) {
      // Quoted, so a path fragment or a word in prose does not trip it — the
      // violation being hunted is code that *decides* something per system.
      if (!line.includes(`"${id}"`)) {
        continue;
      }
      const known = KNOWN.get(relative);
      if (known && known.id === id) {
        stale.delete(relative);
        continue;
      }
      failures.push(`${relative}:${index + 1} names "${id}"`);
    }
  });
}

if (failures.length > 0) {
  process.stdout.write(
    `[system-registry] shared server code must not name a game system.\n` +
      `A pack declares what it contributes; nothing here lists them.\n\n`,
  );
  for (const failure of failures) {
    process.stdout.write(`  ${failure}\n`);
  }
  process.stdout.write(
    `\nIf this is a genuine exception, it belongs in system_packs.rs with a\n` +
      `reason, not behind a widened check. See spec 032 FR-029 and ADR-061.\n`,
  );
  process.exit(1);
}

// A known violation that has been fixed must leave the list, or the list
// becomes a place exceptions go to be forgotten.
if (stale.size > 0) {
  process.stdout.write(
    `[system-registry] these no longer violate anything and should be removed\n` +
      `from KNOWN in this script:\n`,
  );
  for (const entry of stale) {
    process.stdout.write(`  ${entry}\n`);
  }
  process.exit(1);
}

process.stdout.write(
  `[system-registry] shared server code names none of: ${ids.join(", ")}\n` +
    (KNOWN.size === 0
      ? `                  and nothing is exempted.\n`
      : `                  (${KNOWN.size} known violation(s) outstanding)\n`),
);
