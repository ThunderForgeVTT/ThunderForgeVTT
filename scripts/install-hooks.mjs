#!/usr/bin/env node
/**
 * Points this clone's git hooks at the tracked `.hooks/` directory.
 *
 * # Why this is a script and not a line in the README
 *
 * `.git/hooks/` is not tracked, and nothing in a clone is. Hooks committed
 * to a repository do nothing at all until each clone is told where they
 * live, which means the honest choice is between a setup step everyone runs
 * without being asked and a setup step in a document that half the clones
 * will skip. `prepare` in package.json runs on `pnpm install`, which is the
 * first thing anyone does here anyway, so the hooks arrive with the
 * dependencies rather than as a thing to remember.
 *
 * `core.hooksPath` is per-clone local config. It has to be, so this runs
 * every install and simply re-asserts the value if it is already right.
 *
 * # Not a lock
 *
 * `git commit --no-verify` still works, and so does unsetting this. That is
 * deliberate. These hooks are a convenience that catches formatting before
 * it reaches CI; treating them as enforcement would be a mistake, because
 * the only enforcement that counts runs somewhere the author cannot switch
 * off.
 */

import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, readdirSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const HOOKS_PATH = ".hooks";

const git = (...args) =>
  spawnSync("git", args, { cwd: repoRoot, encoding: "utf8" });

// A tarball, a vendored copy, or a docker build context is not a git repo.
// Installing dependencies there must not fail over a hook.
if (git("rev-parse", "--git-dir").status !== 0) {
  process.stdout.write("hooks: not a git repository — skipping.\n");
  process.exit(0);
}

if (!existsSync(path.join(repoRoot, HOOKS_PATH))) {
  process.stdout.write(`hooks: ${HOOKS_PATH}/ is missing — skipping.\n`);
  process.exit(0);
}

// Git silently skips a hook that is not executable. The bit is tracked, but a
// checkout with `core.fileMode=false` or an archive extraction can drop it,
// and a hook that does not run looks exactly like a hook that passed.
for (const name of readdirSync(path.join(repoRoot, HOOKS_PATH))) {
  const file = path.join(repoRoot, HOOKS_PATH, name);
  const mode = statSync(file).mode;
  if ((mode & 0o111) === 0) chmodSync(file, mode | 0o755);
}

const LISTING =
  "  pre-commit     formatters and the two static checks (~4s)\n" +
  "  pre-push       the full `pnpm verify`\n" +
  "  post-merge     warns when pnpm-lock.yaml changed and node_modules is stale\n" +
  "  post-checkout  the same, after a branch switch\n" +
  "  bypass         --no-verify on commit and push\n";

const current = git("config", "--local", "core.hooksPath").stdout.trim();
if (current === HOOKS_PATH) {
  process.stdout.write(`hooks: already pointed at ${HOOKS_PATH}/.\n`);
  process.exit(0);
}

const set = git("config", "--local", "core.hooksPath", HOOKS_PATH);
if (set.status !== 0) {
  // Worth reporting, not worth failing an install over.
  process.stdout.write(
    `hooks: could not set core.hooksPath (${set.stderr.trim()}).\n`,
  );
  process.exit(0);
}

process.stdout.write(
  `hooks: core.hooksPath → ${HOOKS_PATH}/${
    current ? ` (was "${current}")` : ""
  }\n${LISTING}`,
);
