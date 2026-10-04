#!/usr/bin/env node
/**
 * Spec 065, FR-007 to FR-009: the layout, written as a check anyone can run.
 *
 * The rule is short. Something a person runs lives in `apps/`. A Rust
 * library lives in `crates/`. Shared web code lives in `packages/`. A game
 * system lives in `packs/systems/<id>/`. Nothing else is a home, and no app
 * imports from another app.
 *
 * Until 2026-10-04 the four most important crates in the project sat under a
 * root `src/`, outside every home. Nobody decided that; it is where the first
 * crate was put, and the next three followed it. Moving them is worth little
 * if the next crate does the same, so this refuses it in the commit that adds
 * it — the same reason the file-length and e2e-slice rules are checks rather
 * than conventions.
 *
 * # Why it reads files instead of asking cargo
 *
 * `cargo metadata` would answer the question exactly, and takes seconds on a
 * cold tree. This runs before every commit, so it reads the workspace
 * manifest and each member's own, which is enough: a binary is a `[[bin]]`
 * table, a `src/main.rs`, or a `src/bin/`.
 *
 * # Exceptions are named, with the reason beside them
 *
 * A member that breaks the bin/lib rule on purpose is listed in
 * `ALLOWED_MISFITS` below. An entry that no longer applies fails the check
 * too, so the list cannot outlive what it excuses.
 */

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

/** Where a Rust workspace member may live. */
export const RUST_ROOTS = ["apps/", "crates/", "packs/systems/"];

/**
 * Members that break the bin/lib rule on purpose: `path -> reason`.
 *
 * `bin-in-crates` is a library that also ships a binary; `lib-in-apps` is an
 * entry point with no binary target of its own.
 */
export const ALLOWED_MISFITS = new Map([]);

/**
 * App-to-app imports that exist today: `"from-app -> to-app" -> reason`.
 *
 * Spec 065 Story 4 moves what these reach for into `packages/`. Until then
 * each pair is named here, so a new pair is refused while the known ones
 * are worked down.
 */
export const ALLOWED_CROSS_APP = new Map([]);

/** The `members = [...]` list of a workspace manifest. */
export function workspaceMembers(manifest) {
  const block = /^\s*members\s*=\s*\[([\s\S]*?)\]/m.exec(manifest);
  if (!block) return [];
  return [...block[1].matchAll(/"([^"]+)"/g)].map((match) => match[1]);
}

/** Whether the crate at `dir` (absolute) builds a binary. */
export function declaresBinary(dir) {
  const manifest = readFileSync(path.join(dir, "Cargo.toml"), "utf8");
  return (
    /^\s*\[\[bin\]\]/m.test(manifest) ||
    existsSync(path.join(dir, "src", "main.rs")) ||
    existsSync(path.join(dir, "src", "bin"))
  );
}

/** Problems with where the Rust workspace members live. */
export function rustProblems(root, misfits = ALLOWED_MISFITS) {
  const members = workspaceMembers(
    readFileSync(path.join(root, "Cargo.toml"), "utf8"),
  );
  const problems = [];
  const used = new Set();
  for (const member of members) {
    if (!RUST_ROOTS.some((home) => member.startsWith(home))) {
      problems.push(
        `${member}: a workspace member must live under ${RUST_ROOTS.join(", ")} ` +
          `(binaries in apps/, libraries in crates/, game systems in packs/systems/<id>/)`,
      );
      continue;
    }
    const dir = path.join(root, member);
    if (!existsSync(path.join(dir, "Cargo.toml"))) {
      problems.push(`${member}: listed as a member but has no Cargo.toml`);
      continue;
    }
    const binary = declaresBinary(dir);
    let misfit = null;
    if (member.startsWith("apps/") && !binary) {
      misfit = "apps/ is for entry points and this crate builds no binary";
    } else if (member.startsWith("crates/") && binary) {
      misfit = "crates/ is for libraries and this crate builds a binary";
    }
    if (!misfit) continue;
    if (misfits.has(member)) used.add(member);
    else problems.push(`${member}: ${misfit}`);
  }
  for (const member of misfits.keys()) {
    if (!used.has(member)) {
      problems.push(
        `${member}: listed in ALLOWED_MISFITS but no longer breaks the rule; remove the entry`,
      );
    }
  }
  return { members: members.length, problems };
}

const IMPORT =
  /(?:\bfrom\s*|\bimport\s*\(\s*|\bimport\s+|\brequire\s*\(\s*|@import\s+(?:url\(\s*)?)["']([^"']+)["']/g;

/** The app a repo-relative path belongs to, or null. */
function appOf(file) {
  const match = /^apps\/([^/]+)\//.exec(file);
  return match ? match[1] : null;
}

/**
 * Imports that reach from one app into another.
 *
 * `files` is `[repo-relative path, contents]` pairs; `appPackages` maps a
 * package name to the app that owns it, so `import "@x/web"` from another
 * app counts as much as `../../web/src`.
 */
export function crossAppImports(files, appPackages = new Map()) {
  // A relative path that climbs out of one app only counts when it lands in
  // another: `apps/src/...` is a wrong path, not a second app.
  const apps = new Set(files.map(([file]) => appOf(file)).filter(Boolean));
  const found = [];
  for (const [file, contents] of files) {
    const from = appOf(file);
    if (!from) continue;
    for (const match of contents.matchAll(IMPORT)) {
      const specifier = match[1];
      let to = null;
      if (specifier.startsWith(".")) {
        const target = path.posix.normalize(
          path.posix.join(path.posix.dirname(file), specifier),
        );
        to = appOf(`${target}/`);
        if (!apps.has(to)) to = null;
      } else {
        for (const [name, app] of appPackages) {
          if (specifier === name || specifier.startsWith(`${name}/`)) to = app;
        }
      }
      if (to && to !== from) found.push({ file, specifier, from, to });
    }
  }
  return found;
}

/** Problems with imports between apps, given what is allowed for now. */
export function crossAppProblems(found, allowed = ALLOWED_CROSS_APP) {
  const problems = [];
  const used = new Set();
  for (const { file, specifier, from, to } of found) {
    const pair = `${from} -> ${to}`;
    if (allowed.has(pair)) used.add(pair);
    else {
      problems.push(
        `${file}: imports "${specifier}" from apps/${to}; an app may not import ` +
          `from another app — move what it needs into packages/`,
      );
    }
  }
  for (const pair of allowed.keys()) {
    if (!used.has(pair)) {
      problems.push(
        `${pair}: listed in ALLOWED_CROSS_APP but no such import remains; remove the entry`,
      );
    }
  }
  return problems;
}

const SOURCE = /\.(?:[cm]?[jt]sx?|css)$/;

function trackedAppSources(root) {
  const listed = execFileSync("git", ["ls-files", "-z", "--", "apps"], {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  return listed
    .split("\0")
    .filter((file) => SOURCE.test(file) && existsSync(path.join(root, file)));
}

function appPackageNames(root, files) {
  const names = new Map();
  for (const app of new Set(files.map(appOf))) {
    const manifest = path.join(root, "apps", app, "package.json");
    if (!existsSync(manifest)) continue;
    const { name } = JSON.parse(readFileSync(manifest, "utf8"));
    if (name) names.set(name, app);
  }
  return names;
}

function main() {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
  const rust = rustProblems(root);
  const sources = trackedAppSources(root);
  const found = crossAppImports(
    sources.map((file) => [file, readFileSync(path.join(root, file), "utf8")]),
    appPackageNames(root, sources),
  );
  const problems = [...rust.problems, ...crossAppProblems(found)];
  if (existsSync(path.join(root, "src"))) {
    problems.push("src/: the repository root has no src/ directory");
  }
  if (problems.length > 0) {
    for (const problem of problems) process.stderr.write(`[layout] ${problem}\n`);
    process.stderr.write(
      `\n${problems.length} layout problem(s). See specs/065-a-place-for-everything/spec.md.\n`,
    );
    process.exit(1);
  }
  process.stdout.write(
    `layout: ${rust.members} Rust workspace members and ${sources.length} app sources ` +
      `follow the rule (${found.length} app-to-app imports, all listed)\n`,
  );
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main();
