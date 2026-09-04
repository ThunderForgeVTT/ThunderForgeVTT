#!/usr/bin/env node
/**
 * SC-003, written as a check anyone can run.
 *
 * Spec 033 says the set of ability types is the union of the application's
 * built-ins and whatever a game system declares, and that adding a type for
 * one system must change **zero** files shared with other systems. That claim
 * is easy to state and easy to erode: the comfortable way to make a new type
 * work is to teach shared code about it — a `match` arm here, a label map
 * there — and every one of those works perfectly until the next system ships.
 *
 * The precedent is `check-system-registry.mjs`, which forbids shared server
 * code from naming a bundled *system*. This is the same rule one level down,
 * applied to the types those systems declare.
 *
 * # What is allowed
 *
 * The four built-ins — `spell`, `feat`, `power`, `talent` — are the
 * application's own and may be named in application code. They are permanently
 * authorable (FR-017) and something has to define them;
 * `src/server/src/ability_vocabulary.rs` is where.
 *
 * Everything a *pack* declares is that pack's business. If `enchantment`
 * appears as a literal in shared server or web code, some shared thing has
 * learned about one ruleset's vocabulary, and the next pack to want a type
 * will find that adding one costs an edit here.
 *
 * # Why the ids come from the packs
 *
 * Same argument `check-system-registry.mjs` makes: a list in this file would
 * go stale the moment a pack declared something new, and a check that has to
 * be updated to keep working is a check that gets updated to pass.
 */

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

/** The application's own types, defined in `ability_vocabulary.rs`. */
const BUILTINS = new Set(["spell", "feat", "power", "talent"]);

/** Every ability type identity any bundled pack declares, minus the built-ins. */
function packDeclaredTypeIds() {
  const systemsDir = path.join(repoRoot, "packs", "systems");
  const ids = new Map(); // id -> [systems that declare it]

  for (const entry of readdirSync(systemsDir)) {
    const manifestPath = path.join(systemsDir, entry, "system.json");
    if (!existsSync(manifestPath)) continue;

    let manifest;
    try {
      manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
    } catch {
      // A malformed manifest is the pack-contract check's problem, not this
      // one's. Skipping keeps one broken pack from masking every other.
      continue;
    }

    const declared = manifest.abilityVocabulary?.types ?? [];
    for (const type of declared) {
      const id = typeof type?.id === "string" ? type.id.toLowerCase() : null;
      if (!id || BUILTINS.has(id)) continue;
      ids.set(id, [...(ids.get(id) ?? []), entry]);
    }
  }
  return ids;
}

/**
 * Shared code: the server library, the composition root, and the web app.
 *
 * Pack directories are excluded — a pack naming its own type is the whole
 * point. Tests are excluded for the reason every check here excludes them: a
 * test asserting "a 5e world offers Enchantments" has to name Enchantments.
 */
function sharedSources() {
  const roots = [
    path.join(repoRoot, "src", "server", "src"),
    path.join(repoRoot, "src", "app", "src"),
    path.join(repoRoot, "apps", "web", "src"),
  ];
  const out = [];
  const walk = (dir) => {
    for (const entry of readdirSync(dir)) {
      const full = path.join(dir, entry);
      if (statSync(full).isDirectory()) {
        if (entry === "__tests__") continue;
        walk(full);
        continue;
      }
      if (!/\.(rs|ts|tsx)$/.test(entry)) continue;
      if (entry.endsWith("_tests.rs")) continue;
      if (/\.test\.tsx?$/.test(entry)) continue;
      out.push(full);
    }
  };
  for (const root of roots) if (existsSync(root)) walk(root);
  return out;
}

/** Strip `#[cfg(test)]` modules, as the sibling checks do. */
function withoutTests(source, file) {
  if (!file.endsWith(".rs")) return source;
  const at = source.indexOf("#[cfg(test)]");
  return at === -1 ? source : source.slice(0, at);
}

const declared = packDeclaredTypeIds();
const failures = [];

for (const file of sharedSources()) {
  const source = withoutTests(readFileSync(file, "utf8"), file);
  const relative = path.relative(repoRoot, file);
  source.split("\n").forEach((line, index) => {
    for (const [id, systems] of declared) {
      // Quoted, so prose and identifiers do not trip it — what is being
      // hunted is code that *decides* something for one system's type.
      if (line.includes(`"${id}"`) || line.includes(`'${id}'`)) {
        failures.push(
          `${relative}:${index + 1} names "${id}", declared by ${systems.join(", ")}`,
        );
      }
    }
  });
}

if (failures.length > 0) {
  process.stdout.write(
    `[ability-vocabulary] shared code must not name a type a game system\n` +
      `declared. A pack owns its vocabulary; nothing here knows it.\n\n`,
  );
  for (const failure of failures) process.stdout.write(`  ${failure}\n`);
  process.stdout.write(
    `\nThe four built-ins (${[...BUILTINS].join(", ")}) are the application's\n` +
      `own and may be named. See spec 033 SC-003 and ADR-064.\n`,
  );
  process.exit(1);
}

process.stdout.write(
  `[ability-vocabulary] shared code names none of the ${declared.size} ` +
    `pack-declared type(s)` +
    (declared.size > 0 ? `: ${[...declared.keys()].join(", ")}\n` : `\n`),
);
