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
 * Shared web code: `apps/web/src`.
 *
 * FR-029 was written about the server, and for a long time the check was too.
 * That was not a considered scope — it was just where the violation had been
 * found. The client half had its own registry doing the same thing, a
 * hand-written `{ genie: GenieActorSheet }` in `systemActorSheets.ts`, and a
 * rule enforced on one half and unenforced on the other is how a rule becomes
 * a thing people remember about the backend.
 *
 * Excluded, for the same reasons as the Rust side: a pack's own web code (a
 * pack naming itself is the point), and tests, which must name a system to
 * assert anything about one.
 */
function sharedWebSources() {
  const root = path.join(repoRoot, "apps", "web", "src");
  const out = [];
  const walk = (dir) => {
    for (const entry of readdirSync(dir)) {
      if (entry === "node_modules" || entry === "__tests__") continue;
      const full = path.join(dir, entry);
      if (statSync(full).isDirectory()) {
        walk(full);
      } else if (/\.tsx?$/.test(entry) && !/\.(test|spec)\.tsx?$/.test(entry)) {
        out.push(full);
      }
    }
  };
  walk(root);
  return out;
}

/**
 * Strip `#[cfg(test)]` modules and test files.
 *
 * A test asserting "a Genie actor derives its Wish Points" must name Genie.
 * The rule is about what the *product* knows, not what its tests do.
 *
 * This used to truncate the file at the first `#[cfg(test)]` and return
 * everything before it, which silently exempted every line after the first
 * test module in the file — including, in `graphql.rs`, about 1,450 lines of
 * shared server code sitting after a *nested* test module at line 2657. One
 * of them named a system, and the check reported the file clean for as long
 * as the two lived together. Splitting that file is what surfaced it.
 *
 * So each `#[cfg(test)]` block is now removed individually, by matching
 * braces from the `{` that opens it, and the code after it is kept and
 * scanned. Braces inside strings, chars and comments are skipped — a `"{"`
 * in a test's assertion message would otherwise swallow the rest of the file
 * and reintroduce exactly the blind spot this replaces.
 */
function withoutTests(source) {
  const marker = "#[cfg(test)]";
  let out = source;
  for (;;) {
    const at = out.indexOf(marker);
    if (at === -1) return out;
    const open = out.indexOf("{", at);
    if (open === -1) return out.slice(0, at);
    const close = matchingBrace(out, open);
    // An unbalanced block means the file does not parse; drop the remainder
    // rather than guessing, which is what the old behaviour did everywhere.
    if (close === -1) return out.slice(0, at);
    out = out.slice(0, at) + out.slice(close + 1);
  }
}

/**
 * Index of the `}` closing the `{` at `open`, or -1.
 *
 * Skips braces inside string literals, char literals, raw strings and
 * comments, because a test message containing a brace is ordinary and
 * miscounting one would silently unscan the rest of the file.
 */
function matchingBrace(text, open) {
  let depth = 0;
  for (let i = open; i < text.length; i++) {
    const c = text[i];
    if (c === "/" && text[i + 1] === "/") {
      const nl = text.indexOf("\n", i);
      if (nl === -1) return -1;
      i = nl;
      continue;
    }
    if (c === "/" && text[i + 1] === "*") {
      const end = text.indexOf("*/", i + 2);
      if (end === -1) return -1;
      i = end + 1;
      continue;
    }
    if (c === "r" && (text[i + 1] === '"' || text[i + 1] === "#")) {
      let hashes = 0;
      let j = i + 1;
      while (text[j] === "#") {
        hashes++;
        j++;
      }
      if (text[j] === '"') {
        const terminator = '"' + "#".repeat(hashes);
        const end = text.indexOf(terminator, j + 1);
        if (end === -1) return -1;
        i = end + terminator.length - 1;
        continue;
      }
    }
    if (c === '"' || c === "'") {
      const quote = c;
      let j = i + 1;
      while (j < text.length) {
        if (text[j] === "\\") {
          j += 2;
          continue;
        }
        if (text[j] === quote) break;
        // A lifetime (`'a`) is not a char literal and has no closing quote.
        if (quote === "'" && text[j] === "\n") break;
        j++;
      }
      if (j >= text.length) return -1;
      i = text[j] === quote ? j : j - 1;
      continue;
    }
    if (c === "{") depth++;
    else if (c === "}") {
      depth--;
      if (depth === 0) return i;
    }
  }
  return -1;
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

// The web half is empty too, as of 2026-09-04.
//
// It held four entries for the length of `032/T108`, all found the day this
// check was widened to cover `apps/web/src`. Each mounted one system's panel
// from a page every system shares: the actor page's NPC shop, the staging
// page's session loop, the settings page's carryover card, and the play
// dock's clocks panel — that last one inverted, printing an empty state for
// every system that was not the named one, which is the same comparison
// wearing the opposite sign.
//
// Retiring them took what the actor sheet took, one level deeper. A pack
// contributes a panel by shipping
// `packs/systems/<id>/web/src/panels/<slot>.tsx`;
// `apps/web/src/panels/systemPanels.ts` globs those at build time and keys
// them `${systemId}:${slot}`; `@thunderforge/host` declares the slot
// vocabulary and one props type per slot, which is the part a sheet did not
// need and a panel does. Two slots may point at one component, and Genie's
// staging and clocks panels do.
//
// The data layer went with them. `api/genieSession.ts`,
// `hooks/useGenieSession.ts` and `engine/world/sync/genieSession.ts` were
// three more files in shared web code named for one system — invisible to
// this check until the filename pass below, because none of them quoted the
// id inside itself. They live in `packs/systems/genie/web/src/session/` now,
// which is possible because ADR-063 already moved that system's tables and
// GraphQL into the pack's server half; a pack owning a schema it could not
// call was the only thing keeping its client in `apps/web`.

// The server half is empty, as of 2026-09-03, and that is the point of the
// list rather than a gap in it.
//
// It held one entry for the whole of spec 032: `graphql.rs` branched on one
// system's name during world creation to insert that system's session row.
// Retiring it took the increment ADR-063 sized — the server became a library
// so a pack could depend on it, and Genie's six tables, eleven models and
// 2,763 lines of GraphQL moved into `packs/systems/genie/server`. The row is
// still written; the pack writes it, through a world-creation hook the server
// runs without knowing whose it is.

/**
 * A path, flattened for comparison against a system id.
 *
 * Ids are written the way a directory is — `year_zero_engine`,
 * `basic-game-system` — and filenames are written the way a component is,
 * `YearZeroEnginePanel.tsx`. Lowercasing alone would miss every id with a
 * separator in it, so both sides lose their separators before they meet.
 * Every bundled id survives that flattening as something distinctive
 * (`yearzeroengine`, `basicgamesystem`, `genie`), so this is not a source of
 * accidental matches.
 */
function flattened(text) {
  return text.toLowerCase().replace(/[_-]/g, "");
}

/**
 * The filename half of the rule.
 *
 * Content matching cannot see this, and the gap was not theoretical: for the
 * length of `032/T108` shared web code held `GenieShopPanel.tsx`,
 * `GenieSessionPanel/`, `useGenieSession.ts`, `api/genieSession.ts` and
 * `engine/world/sync/genieSession.ts`, and not one of them quoted `"genie"`
 * inside itself. Five files named for a single game system, in shared code,
 * invisible to a check written to forbid exactly that — because the check
 * read file *contents*, and a filename is not content.
 *
 * A component can be entirely about one system without ever spelling its id
 * in a string literal. The name is where it says so, so the name is checked.
 */
function namesSystemInPath(relativePath, ids) {
  const haystack = flattened(relativePath);
  return ids.filter((id) => haystack.includes(flattened(id)));
}

const ids = bundledSystemIds();
const failures = [];
const stale = new Set(KNOWN.keys());

for (const file of [...sharedServerSources(), ...sharedWebSources()]) {
  const source = withoutTests(readFileSync(file, "utf8"));
  const relative = path.relative(repoRoot, file);

  for (const id of namesSystemInPath(relative, ids)) {
    const known = KNOWN.get(relative);
    if (known && known.id === id) {
      stale.delete(relative);
      continue;
    }
    failures.push(`${relative} is named for "${id}"`);
  }

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
    `[system-registry] shared server and web code must not name a game system.\n` +
      `A pack declares what it contributes; nothing here lists them.\n\n`,
  );
  for (const failure of failures) {
    process.stdout.write(`  ${failure}\n`);
  }
  process.stdout.write(
    `\nIf this is a genuine exception it goes in the linkage module for its\n` +
      `side — system_packs.rs on the server — with a reason, not behind a\n` +
      `widened check. On the web there is no such module: a pack contributes\n` +
      `by shipping a file the host discovers.\n\n` +
      `  a character sheet   packs/systems/<id>/web/src/ActorSheet.tsx\n` +
      `  a panel             packs/systems/<id>/web/src/panels/<slot>.tsx\n\n` +
      `Slots and their props are declared in @thunderforge/host (PanelSlot,\n` +
      `PanelSlotProps); systemActorSheets.ts and systemPanels.ts are what\n` +
      `find them. See spec 032 FR-029, ADR-061 and ADR-066.\n`,
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
  `[system-registry] no shared server or web file quotes or is named for any\n` +
    `                  of: ${ids.join(", ")}\n` +
    (KNOWN.size === 0
      ? `                  and nothing is exempted.\n`
      : `                  (${KNOWN.size} known violation(s) outstanding)\n`),
);
