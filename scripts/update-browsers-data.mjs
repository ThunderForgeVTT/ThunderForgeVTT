#!/usr/bin/env node
/**
 * Bring `caniuse-lite` — the browser data browserslist reads — up to the
 * registry's latest, touching nothing else in the lockfile.
 *
 * # Why not `npx update-browserslist-db`
 *
 * It runs `pnpm up --depth=9999 caniuse-lite`, which here reports "Already up
 * to date" for caniuse-lite and re-resolves unrelated packages (codemirror,
 * lezer) instead. So this rewrites the one lockfile entry — version and
 * integrity, both from the registry — and lets `pnpm install
 * --frozen-lockfile` prove the result is a lockfile pnpm accepts.
 *
 * Run it when the build says "browsers data (caniuse-lite) is N months old",
 * then commit `pnpm-lock.yaml`. The build itself never runs it: the image
 * installs with a frozen lockfile and must build the same thing twice.
 */
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { ROOT_DIR } from "./shared.mjs";

const LOCKFILE = join(ROOT_DIR, "pnpm-lock.yaml");
const PACKAGE = "caniuse-lite";

function pnpm(...args) {
  return execFileSync("pnpm", args, { cwd: ROOT_DIR, encoding: "utf8" });
}

const lock = readFileSync(LOCKFILE, "utf8");
const current = new Set(
  [...lock.matchAll(/^ {2}caniuse-lite@([\d.]+):$/gm)].map((m) => m[1]),
);
if (current.size !== 1) {
  console.error(
    `Expected one ${PACKAGE} in the lockfile, found ${current.size}; update it by hand.`,
  );
  process.exit(1);
}
const [from] = current;
const to = pnpm("view", PACKAGE, "version").trim();
if (from === to) {
  console.log(`${PACKAGE} is already ${to}.`);
  process.exit(0);
}
const integrity = pnpm("view", `${PACKAGE}@${to}`, "dist.integrity").trim();

const entry = new RegExp(
  `(^ {2}${PACKAGE}@${from.replaceAll(".", "\\.")}:\\n {4}resolution: \\{integrity: )[^}]+\\}`,
  "m",
);
if (!entry.test(lock)) {
  console.error(`Could not find the ${PACKAGE}@${from} resolution entry.`);
  process.exit(1);
}
writeFileSync(
  LOCKFILE,
  lock.replace(entry, `$1${integrity}}`).replaceAll(from, to),
);
pnpm("install", "--frozen-lockfile");
console.log(`${PACKAGE} ${from} -> ${to}. Commit pnpm-lock.yaml.`);
