#!/usr/bin/env node
/**
 * Does `node_modules` match `pnpm-lock.yaml`? Answered in milliseconds.
 *
 * # Why this exists
 *
 * On 2026-09-14 a merge added `@axe-core/playwright` and nobody ran
 * `pnpm install`. The e2e harness then spent three minutes building Rust
 * before both shards died with "Cannot find module", and the first-run lane
 * reported a blank white screenshot and a `toHaveURL` timeout — a symptom that
 * points everywhere except at the missing package.
 *
 * # How it answers quickly
 *
 * Two checks, neither of which asks pnpm anything:
 *
 * 1. pnpm writes a copy of the lockfile it installed from to
 *    `node_modules/.pnpm/lock.yaml`, byte for byte. If the tracked lockfile
 *    differs from that copy, the tree was installed from a different lockfile.
 * 2. Every direct dependency each importer declares in the lockfile has an
 *    entry in that importer's own `node_modules`. This catches a package
 *    removed or renamed by hand, which (1) cannot see.
 *
 * `pnpm install --frozen-lockfile --offline` answers the same question but
 * *does* the install when it can, and takes ~1.5s warm; this takes ~20ms and
 * never writes. Optional dependencies are not checked: they are allowed to be
 * absent on a platform that cannot install them.
 *
 * Run directly (`node scripts/e2e/deps.mjs`) it prints the problem and exits
 * 1, which is how the git hooks use it; `--warn` exits 0 regardless.
 */

import { existsSync, lstatSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const DEFAULT_ROOT = join(fileURLToPath(import.meta.url), "..", "..", "..");

/**
 * The lockfile's `importers:` section as `{ importerPath: [packageName] }`.
 *
 * A line parser rather than a YAML library, because this runs precisely when
 * `node_modules` cannot be trusted to contain one. The section's shape is
 * fixed by pnpm: importer paths at two spaces, dependency groups at four,
 * package names at six.
 */
export function lockfileImporters(lockText) {
  const importers = {};
  let inImporters = false;
  let importer = null;
  let group = null;
  for (const line of lockText.split("\n")) {
    if (/^\S/.test(line)) {
      inImporters = line.startsWith("importers:");
      continue;
    }
    if (!inImporters) continue;
    let match;
    if ((match = /^ {2}([^\s].*):(?: \{\})?$/.exec(line))) {
      importer = match[1].replace(/^'(.*)'$/, "$1");
      importers[importer] = [];
      group = null;
    } else if ((match = /^ {4}(\w+):$/.exec(line))) {
      group = match[1];
    } else if (
      importer &&
      (group === "dependencies" || group === "devDependencies") &&
      (match = /^ {6}([^\s].*):$/.exec(line))
    ) {
      importers[importer].push(match[1].replace(/^'(.*)'$/, "$1"));
    }
  }
  return importers;
}

/** The problems found, or an empty list when `node_modules` is current. */
export function checkDependencies(root = DEFAULT_ROOT) {
  const lockPath = join(root, "pnpm-lock.yaml");
  if (!existsSync(lockPath)) return [];
  const lockText = readFileSync(lockPath, "utf-8");

  const installedLock = join(root, "node_modules", ".pnpm", "lock.yaml");
  if (!existsSync(installedLock)) {
    return ["node_modules is missing or was not installed by pnpm"];
  }
  const problems = [];
  if (readFileSync(installedLock, "utf-8") !== lockText) {
    problems.push(
      "pnpm-lock.yaml has changed since node_modules was last installed",
    );
  }

  const missing = [];
  for (const [importer, packages] of Object.entries(
    lockfileImporters(lockText),
  )) {
    const importerDir = join(root, importer);
    // A lockfile importer whose directory is not in this checkout (a build
    // output such as `dist/engine` before its first build) has nothing to
    // resolve from yet.
    if (!existsSync(join(importerDir, "package.json"))) continue;
    for (const name of packages) {
      try {
        // `lstat`, not `exists`: a workspace link to a not-yet-built package
        // is a dangling symlink, and that is installed, not missing.
        lstatSync(join(importerDir, "node_modules", name));
      } catch {
        missing.push(`${importer === "." ? "" : `${importer}: `}${name}`);
      }
    }
  }
  if (missing.length) {
    const shown = missing.slice(0, 5).join(", ");
    const more = missing.length > 5 ? ` and ${missing.length - 5} more` : "";
    problems.push(`not installed: ${shown}${more}`);
  }
  return problems;
}

/** One paragraph a person can act on. */
export function describeProblems(problems) {
  return (
    `node_modules does not match pnpm-lock.yaml — run \`pnpm install\`.\n` +
    problems.map((p) => `  - ${p}`).join("\n")
  );
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const warnOnly = process.argv.includes("--warn");
  const started = performance.now();
  const problems = checkDependencies();
  const ms = (performance.now() - started).toFixed(0);
  if (problems.length === 0) {
    if (!warnOnly)
      process.stdout.write(`deps: node_modules is current (${ms}ms).\n`);
    process.exit(0);
  }
  process.stderr.write(`deps: ${describeProblems(problems)}\n`);
  process.exit(warnOnly ? 0 : 1);
}
