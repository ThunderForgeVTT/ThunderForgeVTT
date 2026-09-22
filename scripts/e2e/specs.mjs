/**
 * Which spec files exist, and which lane each one runs in.
 *
 * Moved out of `scripts/e2e-parallel.mjs` (spec 060, T003) so that the slice
 * list (`slices.mjs`), its coverage check and its lookup see *exactly* the
 * file set the runner runs: `e2e/torture` included, `e2e/journeys` excluded.
 * A second walk of `apps/web/e2e`, written separately, would agree with the
 * runner today and quietly disagree the first time either one changed.
 *
 * Nothing here starts a process or touches git; it reads the directory tree
 * and nothing else.
 */

import { readdirSync } from "node:fs";
import { join, relative } from "node:path";

import { ROOT_DIR } from "../shared.mjs";

/**
 * The specs that measure this machine rather than the product.
 *
 * Chosen by reading their assertions, not by name: these are the files that
 * assert on a duration or a frame rate, so a neighbour competing for the GPU
 * changes their result. `engine-limits` gates `fps > 20` across a token sweep,
 * and `canvas-authoring` holds map import under SC-007's 30 seconds.
 *
 * Everything else runs sharded, including the `world-cache` family — its
 * assertions are on item and byte counts (`cacheItems`, `networkItems`), which
 * contention cannot move. That matters: `world-cache-isolated` alone is 6.4
 * minutes of the sweep, and keeping it in the parallel lane is most of the win.
 */
export const PERF_LANE_SPECS = [
  "engine-limits",
  "engine-status-limits",
  "engine-lighting-limits",
  "engine-interaction-limits",
  "engine-loading",
  "canvas-authoring",
  // `status-systems` and `world-cache-isolated` used to be here too. Both had
  // failed in a four-shard run and passed alone, and the explanation was GPU
  // contention. Neither asserts on a duration, and a full run on 2026-09-11,
  // sampled every ten seconds, never saw that contention: the sharded lane's
  // GPU peaked at 49% (p90 35%) with 16GB of memory still free. Keeping them
  // here cost seven minutes of a serial lane that is most of the wall clock,
  // so they are back in the sharded lane — and if they fail there again, the
  // evidence to look for is a saturated GPU, not their names on this list.
];

/**
 * The suites this runner knows. `e2e` is the default and the one every change
 * is held to; `playtest` is run by hand (`pnpm playtest`), on one stack, under
 * its own config — see `apps/web/playwright.playtest.config.ts` for why it is
 * kept apart. `report` is where that config's HTML report goes, since the
 * command line's `--reporter` otherwise replaces the config's own.
 */
export const SUITES = {
  e2e: { dir: "apps/web/e2e", suffix: ".spec.ts", config: null, report: null },
  playtest: {
    dir: "apps/web/playtest",
    suffix: ".playtest.ts",
    config: "playwright.playtest.config.ts",
    report: "playtest-report",
  },
};

/**
 * Every spec file of `suite`, relative to `apps/web`, including `e2e/torture`.
 *
 * Not `e2e/journeys`. A `.journey.spec.ts` ends in `.spec.ts`, but journeys
 * run on an instance of their own (`scripts/journeys.mjs`) and
 * `playwright.config.ts` ignores them — so naming one to a shard selects
 * nothing, and a shard that drew only journeys would fail on "No tests found".
 */
export function allSpecFiles(suite = "e2e") {
  const { dir, suffix } = SUITES[suite];
  const root = join(ROOT_DIR, dir);
  const journeys = join(ROOT_DIR, "apps/web/e2e/journeys");
  const found = [];
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) {
        if (full !== journeys) walk(full);
      } else if (entry.name.endsWith(suffix)) {
        found.push(relative(join(ROOT_DIR, "apps/web"), full));
      }
    }
  };
  walk(root);
  return found.sort();
}

/**
 * The specs that need a database nobody has set up yet.
 *
 * Matched by filename rather than by content: unlike `isPerfSpec`, which reads
 * assertions because "measures the machine" is a property of what a test
 * checks, "needs an unconfigured instance" is a property of which *stack* it
 * must run against, and that is a lane, not a heuristic.
 */
export function isFirstRunSpec(file) {
  return file.endsWith("instance-setup.spec.ts");
}

export function isPerfSpec(file) {
  return PERF_LANE_SPECS.some((name) => file.endsWith(`/${name}.spec.ts`));
}

/**
 * The specs that need GitHub applications configured the *other* way round.
 *
 * Every other shard fixes the feedback application in its environment and
 * leaves sync unset, and `feedback-credentials.spec.ts` is written for exactly
 * that shape. These two need its mirror image, and no single stack can be
 * both:
 *
 * - `github-apps.spec.ts` writes the global and feedback applications through
 *   the screens. An environment-fixed feedback application refuses every
 *   write, so three of its scenarios skipped on every run.
 * - `lore-repository-sync.spec.ts`'s grant hand-off needs an instance that can
 *   connect, and `instanceRepositoryIntegration` reads *only* the environment
 *   (`repo_host::registration_from_env`) — no instance setting makes it true.
 *
 * So they get a stack of their own, which is the same answer the first-run
 * lane already gives to "this spec needs a differently configured instance".
 */
export function isGithubAppsSpec(file) {
  return (
    file.endsWith("/github-apps.spec.ts") ||
    file.endsWith("/lore-repository-sync.spec.ts")
  );
}
