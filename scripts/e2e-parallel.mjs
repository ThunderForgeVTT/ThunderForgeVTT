#!/usr/bin/env node
/**
 * Run the e2e suite as N shards, each against its own stack.
 *
 * # Why this exists
 *
 * The suite takes an hour at `--workers=1`, and `--workers=1` is not a
 * conservative default — it is the only safe setting, because every worker
 * would otherwise share one Postgres database and one RustFS bucket. Tests
 * register users, create worlds and import maps; run two at once against one
 * database and they interfere in ways that read as product bugs.
 *
 * So the thing to isolate is not the *browser* but the state underneath it.
 * Each shard here gets its own database, its own bucket, its own backend and
 * its own frontend. Playwright's `--shard` then splits the suite across them.
 *
 * # Layers, not container stacks
 *
 * One Postgres and one RustFS serve every shard. Postgres hosts many
 * databases at negligible cost, and `storage/rustfs.rs`'s `ensure_bucket`
 * already creates a bucket on demand — so a shard needs a `DATABASE_URL` and
 * a `RUSTFS_BUCKET`, not a container of its own. Starting N full container
 * stacks would pay seconds of startup and hundreds of megabytes per shard to
 * isolate state that a `CREATE DATABASE` already isolates.
 *
 * The database is cloned from a template that is migrated and seeded **once**
 * (`CREATE DATABASE ... TEMPLATE ...`), because running the full migration
 * chain per shard would put the cost back that this script exists to remove.
 *
 * # What is deliberately not sharded
 *
 * The engine benchmarks. `engine-limits.spec.ts` asserts `fps > 20` and that
 * every swept level yields a real reading; `world-cache` compares a cold visit
 * against a warm one. Those measure this machine, so running them beside three
 * other shards competing for the same GPU makes them measure the neighbours.
 * They run alone, after the sharded lane, unless `--all` says otherwise.
 */

import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { createServer } from "node:net";
import { join, relative } from "node:path";

import {
  ROOT_DIR,
  ensureEngineBuild,
  ensurePdfBuild,
  engineProfile,
  skipWasmOpt,
  log,
  runCommand,
  spawnManaged,
  terminateChildren,
} from "./shared.mjs";
import { checkDependencies, describeProblems } from "./e2e/deps.mjs";
import { acquireRunLock, releaseRunLock } from "./e2e/run-lock.mjs";
import { removeContainers, startMailpitContainer } from "./e2e/mailpit.mjs";
import { startLoadMonitor } from "./e2e/load.mjs";
import { digestLines, writeRunSummary } from "./e2e/report.mjs";

/** Away from 5173/30000 on purpose, so a `pnpm dev` can stay up while this runs. */
const WEB_PORT_BASE = 5200;
const BACKEND_PORT_BASE = 30100;
// One Mailpit per shard, exactly as backends, vite servers and buckets already
// get one (contracts/e2e-fixtures.md § 2 rule 2). Sharing the dev stack's
// single sink would make "the newest message" mean whichever shard sent last.
const MAILPIT_SMTP_PORT_BASE = 31025;
const MAILPIT_API_PORT_BASE = 38025;
const MAILPIT_IMAGE = "axllent/mailpit:v1.21";
// Spec 037: a GitHub that is not GitHub, one per shard. The backend reaches it
// through `GITHUB_API_BASE`, which is the same configuration value an operator
// running GitHub Enterprise sets — so delivery is exercised through the
// production code path rather than a test branch.
const GITHUB_STUB_PORT_BASE = 31500;
/**
 * Spec 036 US6 (T063): this shard's OAuth provider.
 *
 * A port per shard for the same reason every other service gets one — two
 * shards sharing a provider would share the identity a scenario chose through
 * `/_control/identity`, and one shard's "the provider returned no email" would
 * become another shard's mystery refusal.
 */
const OAUTH_STUB_PORT_BASE = 31600;

const POSTGRES_CONTAINER =
  process.env.THUNDERFORGE_POSTGRES_CONTAINER ?? "thunderforge-postgres";
const DB_USER = process.env.THUNDERFORGE_DB_USER ?? "postgres";
const TEMPLATE_DB = "thunderforge_e2e_template";
/**
 * The other template: migrated, and deliberately **not** seeded.
 *
 * Spec 040 US1 is first-run setup, and first-run is unobservable against the
 * seeded template — `demo_accounts.sql` has already created the platform
 * administrator and marked setup complete, so `/setup` redirects and the whole
 * story is unreachable. That is why no setup spec existed before this: not
 * because nobody wrote one, but because there was nowhere for it to run.
 *
 * Migrations only. Anything a migration inserts is part of an empty
 * deployment by definition and belongs here; anything a *seed* inserts is a
 * convenience for the other lane and would defeat the point of this one.
 */
const FIRST_RUN_TEMPLATE_DB = "thunderforge_e2e_firstrun_template";
const SHARD_DIR = join(ROOT_DIR, ".e2e-shards");
/**
 * Measured seconds per spec file, so each run balances better than the last.
 *
 * Two files. The tracked one is a baseline that keeps the first run on a new
 * machine balanced, and changes only when someone asks (`--record-durations`)
 * and commits it. The local one is rewritten by every run and preferred when
 * present, so this machine's own measurements still steer its next run.
 */
const DURATIONS_PATH = join(ROOT_DIR, ".e2e-shards-durations.json");
const LOCAL_DURATIONS_PATH = join(ROOT_DIR, ".e2e-shards-durations.local.json");

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
const PERF_LANE_SPECS = [
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
 * Which measured specs this invocation will run *at all*.
 *
 * Deliberately ignores `--all`. That flag decides whether the measured specs
 * get a lane of their own, not whether they run — and conflating the two is a
 * mistake this function already made once: a run with `--all` chose a dev
 * build, `canvas-authoring` went into the parallel lane anyway, and its map
 * import blew a 120-second timeout it clears in about 22 on release.
 *
 * Needed early, because the engine profile has to be decided before the build
 * and that is well before the lanes are partitioned below.
 */
function measuredSpecsSelected(args) {
  const onlyPatterns = args.only
    ?.split(",")
    .map((p) => p.trim())
    .filter(Boolean);
  return allSpecFiles(args.suite)
    .filter(
      (file) => !onlyPatterns || onlyPatterns.some((p) => file.includes(p)),
    )
    .filter(isPerfSpec);
}

/**
 * The suites this runner knows. `e2e` is the default and the one every change
 * is held to; `playtest` is run by hand (`pnpm playtest`), on one stack, under
 * its own config — see `apps/web/playwright.playtest.config.ts` for why it is
 * kept apart. `report` is where that config's HTML report goes, since the
 * command line's `--reporter` otherwise replaces the config's own.
 */
const SUITES = {
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
function allSpecFiles(suite = "e2e") {
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
function isFirstRunSpec(file) {
  return file.endsWith("instance-setup.spec.ts");
}

function isPerfSpec(file) {
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
function isGithubAppsSpec(file) {
  return (
    file.endsWith("/github-apps.spec.ts") ||
    file.endsWith("/lore-repository-sync.spec.ts")
  );
}

/**
 * That stack's applications: feedback blank so it is writable, sync from the
 * environment so lore sync can connect. The key is the throwaway fixture the
 * feedback application uses elsewhere; the host behind it is the shard's
 * GitHub stub, through `GITHUB_API_BASE` and `GITHUB_WEB_BASE`.
 */
const GITHUB_APPS_LANE_ENV = {
  FEEDBACK_GITHUB_APP_CLIENT_ID: "",
  FEEDBACK_GITHUB_APP_SLUG: "",
  FEEDBACK_GITHUB_APP_PRIVATE_KEY_FILE: "",
  SYNC_GITHUB_APP_CLIENT_ID: "Iv1.e2esyncstub",
  SYNC_GITHUB_APP_SLUG: "thunderforge-sync-stub",
  SYNC_GITHUB_APP_PRIVATE_KEY_FILE: join(
    ROOT_DIR,
    "crates/thunderforge-repo-host/tests/fixtures/throwaway-test-app-key.pem",
  ),
};

/**
 * Seconds per spec file: this machine's last run if it has one, else the
 * committed baseline, else `{}`.
 */
function readDurations() {
  for (const path of [LOCAL_DURATIONS_PATH, DURATIONS_PATH]) {
    try {
      return JSON.parse(readFileSync(path, "utf-8"));
    } catch {
      // Absent or unreadable; try the next.
    }
  }
  return {};
}

/**
 * Split spec files across shards by how long they take, longest first.
 *
 * Playwright's own `--shard` divides by test count, which is the wrong measure
 * when one file is a quarter of the suite: the first sharded run finished its
 * lightest shard in 6.0 minutes and its heaviest in 21.2, so three quarters of
 * the machine sat idle while `token-authoring.spec.ts` finished alone.
 *
 * Longest-processing-time-first: repeatedly give the next-largest file to the
 * shard with the least work so far. It is the standard greedy approximation
 * and lands within a few percent of optimal here, where one file dominates.
 *
 * A file with no recorded duration is assumed average rather than zero, so a
 * newly added spec is distributed rather than piled onto one shard.
 */
function partitionByDuration(files, shardCount, durations) {
  const known = files
    .map((f) => durations[f])
    .filter((d) => typeof d === "number");
  const fallback =
    known.length > 0 ? known.reduce((a, b) => a + b, 0) / known.length : 1;
  const weighted = files
    .map((file) => ({ file, cost: durations[file] ?? fallback }))
    .sort((a, b) => b.cost - a.cost);

  const bins = Array.from({ length: shardCount }, () => ({
    cost: 0,
    files: [],
  }));
  for (const { file, cost } of weighted) {
    const lightest = bins.reduce((a, b) => (a.cost <= b.cost ? a : b));
    lightest.files.push(file);
    lightest.cost += cost;
  }
  return bins;
}

/**
 * Fold this run's per-test timings back into the durations file.
 *
 * Written from Playwright's own JSON report rather than hand-timed, and only
 * for shards that produced one — a crashed shard must not zero out the
 * estimate that keeps the next run balanced.
 *
 * Always to the gitignored local file; to the tracked baseline only under
 * `--record-durations`. Every run used to rewrite the tracked file, so every
 * checkout that had run e2e had a modified file in git, and on 2026-09-14 that
 * local rewrite blocked a fast-forward merge.
 */
function recordDurations(shardDirs, previous, { baseline = false } = {}) {
  const totals = { ...previous };
  const reports = shardDirs.flatMap((dir) => [
    join(dir, "results-parallel.json"),
    join(dir, "results-serial.json"),
  ]);
  for (const path of reports) {
    let report;
    try {
      report = JSON.parse(readFileSync(path, "utf-8"));
    } catch {
      continue;
    }
    const walk = (suite, file) => {
      const path = suite.file ?? file;
      for (const spec of suite.specs ?? []) {
        for (const test of spec.tests ?? []) {
          for (const result of test.results ?? []) {
            totals[`e2e/${path}`] =
              (totals[`e2e/${path}`] ?? 0) + (result.duration ?? 0) / 1000;
          }
        }
      }
      for (const child of suite.suites ?? []) walk(child, path);
    };
    // A fresh total per file, not an accumulation across runs.
    for (const suite of report.suites ?? []) {
      for (const spec of suite.specs ?? []) {
        void spec;
      }
      totals[`e2e/${suite.file}`] = 0;
    }
    for (const suite of report.suites ?? []) walk(suite, suite.file);
  }
  const text = JSON.stringify(totals, null, 2);
  writeFileSync(LOCAL_DURATIONS_PATH, text);
  if (baseline) {
    writeFileSync(DURATIONS_PATH, text);
    log("e2e", `Recorded durations to ${relative(ROOT_DIR, DURATIONS_PATH)}.`);
  }
}

/**
 * Fail loudly if anything already holds a port this run needs.
 *
 * The lock above stops a *second run* starting while a first is alive. It
 * does not stop a first run's **children** outliving it: kill the runner and
 * its backends, vite servers and stubs keep their ports. The next run then
 * starts, fails to bind, and — because readiness is a `fetch` that the corpse
 * answers perfectly well — decides the stack came up and runs the whole suite
 * against the *previous build*. That is worse than a crash: it produced a
 * green run of code that was never compiled, and a red one for a fix that was
 * already in.
 *
 * So bind rather than fetch. A port that cannot be bound is somebody else's,
 * whatever answers on it.
 */
async function assertPortsFree(total) {
  const wanted = [];
  for (let index = 0; index < total; index += 1) {
    wanted.push([WEB_PORT_BASE + index, `vite ${index}`]);
    wanted.push([BACKEND_PORT_BASE + index, `backend ${index}`]);
    wanted.push([GITHUB_STUB_PORT_BASE + index, `github stub ${index}`]);
    wanted.push([OAUTH_STUB_PORT_BASE + index, `oauth stub ${index}`]);
    wanted.push([MAILPIT_SMTP_PORT_BASE + index, `mailpit smtp ${index}`]);
    wanted.push([MAILPIT_API_PORT_BASE + index, `mailpit api ${index}`]);
  }

  const taken = [];
  for (const [port, name] of wanted) {
    const free = await new Promise((resolve) => {
      const probe = createServer();
      probe.once("error", () => resolve(false));
      probe.once("listening", () => probe.close(() => resolve(true)));
      probe.listen(port, "127.0.0.1");
    });
    if (!free) taken.push(`${port} (${name})`);
  }

  if (taken.length) {
    throw new Error(
      `these ports are already in use: ${taken.join(", ")}. ` +
        "Something from an earlier run is still alive — the runner would " +
        "otherwise test against it rather than against this build. Find it " +
        "with `ss -ltnp` and kill it, then run again.",
    );
  }
}

function psql(database, sql) {
  return execFileSync(
    "docker",
    [
      "exec",
      "-i",
      POSTGRES_CONTAINER,
      "psql",
      "-U",
      DB_USER,
      "-d",
      database,
      "-v",
      "ON_ERROR_STOP=1",
      "-q",
    ],
    { input: sql, encoding: "utf-8" },
  );
}

function psqlFile(database, file) {
  return psql(database, readFileSync(join(ROOT_DIR, file), "utf-8"));
}

function shardDbName(index) {
  return `thunderforge_e2e_${index}`;
}

/**
 * Migrate and seed one template database, then clone it per shard.
 *
 * Dropped and rebuilt every run rather than reused: a template that survives a
 * migration being added is a template that silently omits it, and a shard
 * cloned from it fails for a reason no one would look for here.
 */
async function provisionTemplate() {
  log("e2e", `Building the template database (${TEMPLATE_DB})...`);
  // No connections may exist to a database being used as a template, and
  // `WITH (FORCE)` covers a previous run that died holding one.
  psql("postgres", `DROP DATABASE IF EXISTS ${TEMPLATE_DB} WITH (FORCE);`);
  psql("postgres", `CREATE DATABASE ${TEMPLATE_DB};`);

  const templateUrl = `postgres://${DB_USER}:password@localhost:5432/${TEMPLATE_DB}`;
  await runCommand("diesel migration run", {
    name: "migrate template",
    cwd: join(ROOT_DIR, "src/server"),
    prefix: "e2e",
    env: { DATABASE_URL: templateUrl },
  });

  // Both seeds, matching what a dev stack has: `make dev` runs `seed`
  // (demo_accounts) and Playwright's own global setup applies e2e_demo. The
  // latter is idempotent and will run again per shard; having it here too
  // means a shard starts from the same place a developer's stack does.
  psqlFile(TEMPLATE_DB, "src/server/seeds/demo_accounts.sql");
  psqlFile(TEMPLATE_DB, "src/server/seeds/e2e_demo.sql");
  log("e2e", "Template ready.");
}

/**
 * The same migration chain, and none of the seeds.
 *
 * Built only when a first-run spec is actually selected, because it costs a
 * full `diesel migration run` and almost every run of this script has nothing
 * to do with setup.
 */
async function provisionFirstRunTemplate() {
  log("e2e", `Building the first-run template (${FIRST_RUN_TEMPLATE_DB})...`);
  psql(
    "postgres",
    `DROP DATABASE IF EXISTS ${FIRST_RUN_TEMPLATE_DB} WITH (FORCE);`,
  );
  psql("postgres", `CREATE DATABASE ${FIRST_RUN_TEMPLATE_DB};`);

  await runCommand("diesel migration run", {
    name: "migrate first-run template",
    cwd: join(ROOT_DIR, "src/server"),
    prefix: "e2e",
    env: {
      DATABASE_URL: `postgres://${DB_USER}:password@localhost:5432/${FIRST_RUN_TEMPLATE_DB}`,
    },
  });

  log("e2e", "First-run template ready (migrated, unseeded).");
}

/**
 * Every shard database this run created, whether or not its stack came up, so
 * teardown drops the ones a failed start left behind too.
 */
const clonedDatabases = new Set();

function cloneShardDatabase(index, { firstRun = false } = {}) {
  const name = shardDbName(index);
  const template = firstRun ? FIRST_RUN_TEMPLATE_DB : TEMPLATE_DB;
  psql("postgres", `DROP DATABASE IF EXISTS ${name} WITH (FORCE);`);
  psql("postgres", `CREATE DATABASE ${name} TEMPLATE ${template};`);
  clonedDatabases.add(name);

  // Spec 036 US6 (T064). The seed writes the base port because a `.sql` file
  // cannot know which shard it is being applied to; this is where it finds
  // out. Pointing every shard at one stub would mean the identity one
  // scenario chose through `/_control/identity` arrived in another shard's
  // sign-in, and the failure would read as a provisioning bug.
  if (!firstRun) {
    const stub = `http://127.0.0.1:${OAUTH_STUB_PORT_BASE + index}`;
    psql(
      name,
      `UPDATE oauth_providers
          SET authorization_url = '${stub}/authorize',
              token_url = '${stub}/token',
              userinfo_url = '${stub}/userinfo'
        WHERE provider_key = 'stub';`,
    );
  }
  return name;
}

/**
 * Give a shard the installed packs, the way a dev stack has them.
 *
 * `config/mod.rs` resolves `systems_dir` as `<data path>/packs/systems` and
 * `interface_packs_dir` as `<data path>/packs/interface`, and the server reads
 * a pack's manifest from under those. A shard pointed at a fresh data path
 * therefore has *nothing installed* — and the failure never mentions a
 * directory. For systems: `updateWorldGameSystem` succeeds, a token is
 * created, and `tokenAttributes` returns nothing, which surfaces as "genie
 * must resolve attributes for its token". For interface packs: the picker
 * renders with an empty list and the world's look silently stays the
 * stylesheet default, which reads as a broken component.
 *
 * Both were found the same way — by an e2e failing against a real stack — and
 * the second one is why this takes a list rather than naming one directory.
 *
 * Symlinks rather than copies, which is exactly what `data/packs` already is
 * on a dev machine: the shipping system packs are 110MB, and copying them per
 * shard would cost more than the parallelism saves.
 */
function linkPacks(dataPath) {
  for (const kind of ["systems", "interface"]) {
    const source = join(ROOT_DIR, "packs", kind);
    if (!existsSync(source)) continue;
    const target = join(dataPath, "packs", kind);
    mkdirSync(target, { recursive: true });
    for (const entry of readdirSync(source, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      symlinkSync(join(source, entry.name), join(target, entry.name), "dir");
    }
  }
}

async function waitForUrl(url, name, timeoutMs = 180_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return true;
    } catch {
      // Not up yet.
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  log("e2e", `${name} never became ready at ${url}`, process.stderr);
  return false;
}

/**
 * This shard's mail sink. The container handling — unique names, waiting out
 * leftovers, one retry on a conflict — is in `e2e/mailpit.mjs`, which says why.
 *
 * Returns null if it never becomes ready, which fails the shard rather than
 * leaving mail specs to time out one by one against nothing. Throws when the
 * container cannot be started at all; `startStack` turns that into a failed
 * lane with the reason, not a crashed harness.
 */
const startedMailpits = [];

async function startMailpit(index) {
  const smtpPort = MAILPIT_SMTP_PORT_BASE + index;
  const apiPort = MAILPIT_API_PORT_BASE + index;
  const name = await startMailpitContainer({
    index,
    smtpPort,
    apiPort,
    image: MAILPIT_IMAGE,
    onCreated: (created) => startedMailpits.push(created),
    log: (message) => log("e2e", message, process.stderr),
  });

  const api = `http://127.0.0.1:${apiPort}`;
  if (!(await waitForUrl(`${api}/api/v1/info`, `mailpit ${index}`, 60_000))) {
    return null;
  }
  return { name, smtpPort, apiPort, api };
}

/** Remove every sink this run started, including ones that never came up. */
function stopMailpit() {
  removeContainers(startedMailpits.splice(0));
}

/**
 * The env-configured OAuth providers `auth-providers.spec.ts` is written for.
 *
 * Copied from that file's header, which lists them as the stack it needs. The
 * server reads `OAUTH_*` once at startup and materialises each group into the
 * shard's `oauth_providers` table, and every scenario pinned to a named
 * provider checks that table and skips when the provider is missing — so
 * without these, four of its tests passed by not running, on every run.
 *
 * None of them reaches a network. The spec asserts only on the redirect a
 * provider's *start* endpoint issues, and the Keycloak endpoints are derived
 * from the issuer by string (`oauth_env.rs`), not discovered.
 *
 * `OAUTH_KEYCLOAK_LABEL` is set blank on purpose. `dotenvy` searches parent
 * directories for a `.env`, so a worktree under a developer's checkout
 * inherits theirs — and one that labels its Keycloak would fail the test
 * asserting the preset's own label. Blank is unset to `oauth_env.rs`, and
 * `dotenvy` never overwrites a variable that is already present.
 */
const OAUTH_PROVIDER_FIXTURE = {
  OAUTH_DISCORD_CLIENT_ID: "test_discord_client_id",
  OAUTH_DISCORD_CLIENT_SECRET: "test_discord_client_secret",
  OAUTH_KEYCLOAK_ISSUER_URL: "https://idp.example.com/realms/main",
  OAUTH_KEYCLOAK_CLIENT_ID: "test_kc_id",
  OAUTH_KEYCLOAK_CLIENT_SECRET: "test_kc_secret",
  OAUTH_KEYCLOAK_LABEL: "",
  OAUTH_KEYCLOAK_WORK_ISSUER_URL: "https://work.example.com/realms/main",
  OAUTH_KEYCLOAK_WORK_CLIENT_ID: "test_kc_work_id",
  OAUTH_KEYCLOAK_WORK_CLIENT_SECRET: "test_kc_work_secret",
  OAUTH_KEYCLOAK_WORK_LABEL: "Work SSO",
  OAUTH_MYSERVICE_CLIENT_ID: "test_generic_id",
  OAUTH_MYSERVICE_CLIENT_SECRET: "test_generic_secret",
  OAUTH_MYSERVICE_AUTHORIZATION_URL: "https://myservice.example/auth",
  OAUTH_MYSERVICE_TOKEN_URL: "https://myservice.example/token",
};

/** Starts one shard's backend and frontend, and resolves once both answer. */
async function startShard(
  index,
  { firstRun = false, githubApps = false } = {},
) {
  const database = cloneShardDatabase(index, { firstRun });
  const backendPort = BACKEND_PORT_BASE + index;
  const webPort = WEB_PORT_BASE + index;
  const dataPath = join(SHARD_DIR, `shard-${index}`, "data");
  mkdirSync(dataPath, { recursive: true });
  linkPacks(dataPath);

  const shared = {
    DATABASE_URL: `postgres://${DB_USER}:password@localhost:5432/${database}`,
    RUSTFS_BUCKET: `tf-e2e-${index}`,
    THUNDERFORGE_DATA_PATH: dataPath,
    // The single highest-leverage variable in the whole harness: the per-IP
    // auth limiter accounted for 18 of the 42 failures the last full sweep
    // started from, and every shard registers users from the same IP.
    THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT: "1",
    // One declared setting, fixed in the environment on purpose.
    //
    // `instance-settings.spec.ts` asserts that an environment-fixed setting
    // reports its variable and refuses a write — and it used to find its
    // target by looking for any setting the environment happened to fix,
    // skipping itself when there was none. On this harness there was none, so
    // the refusal was pinned by nothing: the test passed by not running.
    //
    // `realm_name` is the one to fix because it costs nothing to fix. It is
    // `Optional` with no capability, so it stays out of the setup wizard's
    // `required_settings`; nothing in `apps/web/src` reads it; and it is
    // neither of the two keys that spec writes, so its own writable-setting
    // tests are unaffected.
    THUNDERFORGE_REALM_NAME: "ThunderForge (e2e)",
    // Only the first-run stack, and it is doing double duty. The setup link
    // the server prints is what the e2e follows, so it has to be a real URL
    // rather than the bare path an unconfigured instance logs — and setting it
    // is also the only exercise T060's fix gets, since the alternative branch
    // is what every containerised operator sees today.
    ...(firstRun
      ? { THUNDERFORGE_PUBLIC_URL: `http://127.0.0.1:${webPort}` }
      : {}),
    // Postgres allows 100 connections; one backend defaults to a pool of 32,
    // sized per core for a machine running one server. Four shards plus a
    // developer's own `pnpm dev` ask for 160, and the failure is not a slow
    // test — it is `FATAL: sorry, too many clients already` from whatever
    // connects next, including the seed step and `psql` itself.
    //
    // Each shard runs `--workers=1`, so a backend here serves one browser and
    // never needs 32. Eight leaves the whole harness under 40 connections at
    // four shards, with the rest free for diesel, the seeds and a dev stack.
    DATABASE_POOL_MAX_SIZE: "8",
    // Feedback delivery talks to this instead of github.com. Both bases are
    // set: the web base decides the `html_url` a delivered issue is recorded
    // under, and leaving it pointing at github.com would put a real URL on a
    // row that describes a stub.
    GITHUB_API_BASE: `http://127.0.0.1:${GITHUB_STUB_PORT_BASE + index}`,
    GITHUB_WEB_BASE: `http://127.0.0.1:${GITHUB_STUB_PORT_BASE + index}`,
    // The feedback subsystem's own application, so delivery resolves
    // credentials the way spec 040 FR-021 says it must — a whole application,
    // not a client id borrowed from one place and a key from another. The key
    // is the committed throwaway fixture the repo-host crate's own tests use;
    // it is worthless and the README beside it says so.
    // Cleared, deliberately. `dotenvy` loads the developer's `.env` into the
    // backend, and a machine with real `SYNC_GITHUB_APP_*` credentials would
    // resolve lore sync from the environment while a CI machine resolved it
    // from the instance store — so a credential-resolution test would pass or
    // fail depending on whose laptop it ran on. Empty rather than removed:
    // the registry treats a blank variable as unset, and removing one is what
    // `dotenv()` undoes on the next call.
    SYNC_GITHUB_APP_CLIENT_ID: "",
    SYNC_GITHUB_APP_SLUG: "",
    SYNC_GITHUB_APP_PRIVATE_KEY: "",
    SYNC_GITHUB_APP_PRIVATE_KEY_FILE: "",
    SYNC_GITHUB_APP_PRIVATE_KEY_BASE64: "",
    GLOBAL_GITHUB_APP_CLIENT_ID: "",
    GLOBAL_GITHUB_APP_SLUG: "",
    GLOBAL_GITHUB_APP_PRIVATE_KEY: "",
    GLOBAL_GITHUB_APP_PRIVATE_KEY_FILE: "",
    GLOBAL_GITHUB_APP_PRIVATE_KEY_BASE64: "",
    FEEDBACK_GITHUB_APP_CLIENT_ID: "Iv1.e2efeedbackstub",
    FEEDBACK_GITHUB_APP_SLUG: "thunderforge-feedback-stub",
    FEEDBACK_GITHUB_APP_PRIVATE_KEY_FILE: join(
      ROOT_DIR,
      "crates/thunderforge-repo-host/tests/fixtures/throwaway-test-app-key.pem",
    ),
    // Seeded stacks only, like the OAuth stub rewrite in `cloneShardDatabase`:
    // the first-run lane is an instance nobody has configured yet.
    ...(firstRun ? {} : OAUTH_PROVIDER_FIXTURE),
    // Last, so it overrides the feedback and sync values above.
    ...(githubApps ? GITHUB_APPS_LANE_ENV : {}),
  };

  spawnManaged(
    `node scripts/github-stub.mjs ${GITHUB_STUB_PORT_BASE + index}`,
    {
      cwd: ROOT_DIR,
      prefix: `gh${index}`,
    },
  );

  // Spec 036 US6. Started before the backend for no reason other than
  // symmetry: the server never calls it until a test drives a sign-in, and
  // the seeded provider row names its URLs rather than discovering them.
  spawnManaged(`node scripts/oauth-stub.mjs ${OAUTH_STUB_PORT_BASE + index}`, {
    cwd: ROOT_DIR,
    prefix: `oa${index}`,
  });

  // The first-run lane keeps a copy of the backend's output, because the setup
  // link the server prints is the only place the bootstrap code exists in
  // plaintext — the database stores an Argon2 hash of it, deliberately. Which
  // is precisely the operator's own position: they read it out of the log too.
  const backendLog = firstRun
    ? join(SHARD_DIR, `shard-${index}`, "backend.log")
    : null;

  spawnManaged(`./target/debug/thunderforge --port ${backendPort}`, {
    cwd: ROOT_DIR,
    prefix: `be${index}`,
    env: shared,
    logPath: backendLog,
  });
  // `/api/readyz`, not `/readyz`: every backend route is nested under `/api`
  // (`main.rs`), and readiness rather than liveness because it is the one that
  // says the database is reachable — which for a freshly cloned shard database
  // is the fact actually in question.
  if (
    !(await waitForUrl(
      `http://127.0.0.1:${backendPort}/api/readyz`,
      `backend ${index}`,
    ))
  ) {
    return null;
  }

  spawnManaged("pnpm -F @thunderforge/web run dev", {
    cwd: ROOT_DIR,
    prefix: `fe${index}`,
    env: {
      ...shared,
      THUNDERFORGE_WEB_PORT: String(webPort),
      THUNDERFORGE_BACKEND_ORIGIN: `http://127.0.0.1:${backendPort}`,
    },
  });
  if (
    !(await waitForUrl(`http://127.0.0.1:${webPort}/`, `frontend ${index}`))
  ) {
    return null;
  }
  if (
    !(await waitForUrl(
      `http://127.0.0.1:${GITHUB_STUB_PORT_BASE + index}/_control/issues`,
      `github stub ${index}`,
      30_000,
    ))
  ) {
    return null;
  }
  if (
    !(await waitForUrl(
      `http://127.0.0.1:${OAUTH_STUB_PORT_BASE + index}/_control/calls`,
      `oauth stub ${index}`,
      30_000,
    ))
  ) {
    return null;
  }

  // Started after the stack rather than before it: the instance is configured
  // to talk to this sink by a *test*, through the settings mutation, which is
  // the whole of what Scenario D is about. Nothing in `shared` above names it.
  const mailpit = await startMailpit(index);
  if (!mailpit) return null;

  return {
    index,
    database,
    webPort,
    backendPort,
    firstRun,
    backendLog,
    mailpit,
  };
}

/** Runs one Playwright shard against an already-started stack. */
function runShard(shard, files, label = "parallel", suite = null) {
  // Which Playwright project this lane is. The projects are a partition of the
  // suite by *stack*, not by browser: `first-run` matches only the setup spec
  // and `chromium` ignores it, so neither lane can pick up the other's files
  // even when someone names them positionally.
  const project = shard.firstRun ? "first-run" : "chromium";
  // An empty list is not "run nothing" to Playwright — `playwright test` with
  // no positional arguments runs the *entire* suite. So a shard that legitimately
  // drew no files (a small `--only`, more shards than specs) would quietly run
  // everything, on every empty shard at once. This cost a real 3-minute run
  // that looked like a hang before anyone noticed what it was doing.
  if (files.length === 0) {
    log("e2e", `  ${label} lane, shard ${shard.index}: no files, skipped.`);
    return Promise.resolve({
      index: shard.index,
      label,
      code: 0,
      failed: false,
      reasons: [],
      skipped: true,
    });
  }

  const demoDir = join(SHARD_DIR, `shard-${shard.index}`, "demo");
  mkdirSync(demoDir, { recursive: true });
  const reportPath = join(
    SHARD_DIR,
    `shard-${shard.index}`,
    `results-${label}.json`,
  );

  // File paths rather than `--grep`: Playwright matches `--grep` against the
  // test *title*, so selecting by filename that way depends on titles happening
  // to mention their file. Positional arguments are matched against the path,
  // which is what is actually meant here — and naming the files makes the two
  // lanes provably a partition, since they come from one list.
  //
  // No `--shard`: the split is done here, by measured duration
  // (`partitionByDuration`), because Playwright's own divides by test count and
  // cannot know that one file is a quarter of the suite.
  //
  // A suite with a config of its own keeps that config's output directory and
  // adds its HTML report; the JSON beside it is what `judgeLane` reads either
  // way.
  const command = suite?.config
    ? `pnpm exec playwright test ${files.join(" ")} --config=${suite.config}` +
      ` --project=${project} --workers=1 --reporter=list,json,html`
    : `pnpm exec playwright test ${files.join(" ")} --project=${project}` +
      ` --workers=1 --reporter=list,json --output=test-results/shard-${shard.index}`;

  const child = spawnManaged(command, {
    cwd: join(ROOT_DIR, "apps/web"),
    prefix: `sh${shard.index}`,
    env: {
      PLAYWRIGHT_BASE_URL: `http://127.0.0.1:${shard.webPort}`,
      THUNDERFORGE_E2E_EXTERNAL_STACK: "1",
      THUNDERFORGE_E2E_DEMO_DIR: demoDir,
      THUNDERFORGE_DB_NAME: shard.database,
      THUNDERFORGE_E2E_GITHUB_STUB: `http://127.0.0.1:${GITHUB_STUB_PORT_BASE + shard.index}`,
      THUNDERFORGE_E2E_OAUTH_STUB: `http://127.0.0.1:${OAUTH_STUB_PORT_BASE + shard.index}`,
      THUNDERFORGE_E2E_MAILPIT_API: shard.mailpit.api,
      THUNDERFORGE_E2E_MAILPIT_SMTP_PORT: String(shard.mailpit.smtpPort),
      // Global setup applies `e2e_demo.sql` and then signs in as the demo user
      // to capture a reusable storage state. Against an unseeded database
      // there is no demo user to sign in as, so it would fail before the first
      // test ran — and seeding to fix that would destroy the very condition
      // this lane exists to reproduce.
      ...(shard.firstRun
        ? {
            THUNDERFORGE_E2E_FIRST_RUN: "1",
            THUNDERFORGE_E2E_BACKEND_LOG: shard.backendLog,
          }
        : {}),
      // Playwright names the JSON report by env var, not by flag. Labelled
      // because shard 0 runs twice — its share of the parallel lane, then the
      // measured lane alone — and a single name meant the second run erased
      // the first. That silently dropped `token-authoring` from the recorded
      // durations, which is the file the whole partition is built around.
      PLAYWRIGHT_JSON_OUTPUT_NAME: reportPath,
      ...(suite?.report
        ? {
            PLAYWRIGHT_HTML_OUTPUT_DIR: suite.report,
            PLAYWRIGHT_HTML_OPEN: "never",
          }
        : {}),
    },
  });

  return new Promise((resolve) => {
    child.once("close", (code) =>
      resolve(
        judgeLane({ index: shard.index, label, code: code ?? 1, reportPath }),
      ),
    );
  });
}

/**
 * One lane's verdict, from what Playwright *reported* as well as how it exited.
 *
 * The exit code alone is not enough. Twice on 2026-09-10/11 a measured-lane
 * run printed `1 failed` and the summary still said `shard 0: passed`, and the
 * whole process exited 0 — a red run reported green. How the code was lost is
 * not established: a plain assertion failure in that lane does propagate, so
 * whatever dropped it was particular to those runs. Rather than trust one
 * channel, the JSON report this lane already writes is read back and a
 * failure in either source fails it: a non-zero exit, an unexpected test, a
 * runner error (a spec that would not even load), or no report at all, since
 * a run that left no report proved nothing.
 *
 * `label` travels with the result because shard 0 runs more than once — its
 * share of the sharded lane, then the measured lane — and a summary that says
 * `shard 0` twice cannot say which of the two failed.
 */
function judgeLane({ index, label, code, reportPath }) {
  let report = null;
  try {
    report = JSON.parse(readFileSync(reportPath, "utf-8"));
  } catch {
    // Missing or unreadable, which is itself a failure below.
  }
  const reasons = [];
  if (code !== 0) reasons.push(`exit ${code}`);
  if (!report) {
    reasons.push("no JSON report");
  } else {
    const unexpected = report.stats?.unexpected ?? 0;
    const errors = report.errors?.length ?? 0;
    if (unexpected > 0) reasons.push(`${unexpected} test(s) failed`);
    if (errors > 0) reasons.push(`${errors} runner error(s)`);
  }
  return {
    index,
    label,
    code,
    failed: reasons.length > 0,
    reasons,
    reportPath: report ? reportPath : null,
  };
}

async function main() {
  // Parsed here rather than through `shared.mjs`'s `parseArgs`, which is a
  // fixed-shape parser for the dev/build scripts' three flags and rejects
  // anything else by design.
  const args = {
    shards: 4,
    all: false,
    keep: false,
    recordDurations: false,
    only: null,
    suite: "e2e",
  };
  for (const argv of process.argv.slice(2)) {
    const shardMatch = /^--shards=(\d+)$/.exec(argv);
    const onlyMatch = /^--only=(.+)$/.exec(argv);
    const suiteMatch = /^--suite=(e2e|playtest)$/.exec(argv);
    if (shardMatch) args.shards = Number(shardMatch[1]);
    else if (suiteMatch) args.suite = suiteMatch[1];
    // A substring of the spec path, for exercising the harness itself without
    // waiting out the suite it exists to speed up.
    else if (onlyMatch) args.only = onlyMatch[1];
    else if (argv === "--all") args.all = true;
    else if (argv === "--keep") args.keep = true;
    // Also write the tracked `.e2e-shards-durations.json` baseline.
    else if (argv === "--record-durations") args.recordDurations = true;
    else throw new Error(`Unknown argument: ${argv}`);
  }
  run.args = args;
  // A playtest is one table on one stack. Each scenario already runs three
  // browsers against an engine-heavy scene and records them, so a second
  // shard would only compete with the recording it is making.
  const total = args.suite === "playtest" ? 1 : args.shards;
  if (!Number.isInteger(total) || total < 1) {
    throw new Error(`--shards must be a positive integer, got ${total}`);
  }

  // Before anything that costs time. A stale `node_modules` otherwise costs
  // the whole Rust build and then fails as "Cannot find module" in every
  // shard, or as a blank page in a lane whose spec never mentions a package.
  const dependencyProblems = checkDependencies(ROOT_DIR);
  if (dependencyProblems.length > 0) {
    log("e2e", describeProblems(dependencyProblems), process.stderr);
    process.exit(1);
  }

  acquireRunLock(ROOT_DIR, process.argv.slice(2));
  releaseOnSignals();
  run.load = startLoadMonitor();
  logLoadAtStart(run.load.snapshot());
  // Two past the sharded stacks: the first-run lane runs on index `total` and
  // the GitHub-applications lane on `total + 1`, and a port either finds taken
  // is the same stale-corpse hazard as a shard's.
  await assertPortsFree(total + 2);
  log("e2e", `Preparing ${total} shard${total === 1 ? "" : "s"}.`);
  rmSync(SHARD_DIR, { recursive: true, force: true });
  mkdirSync(SHARD_DIR, { recursive: true });

  // Dev by default; release only when something is going to be measured.
  //
  // A release build is minutes — cargo is about 35 seconds and `wasm-opt`
  // takes the rest — and neither half scales with cores, because
  // `codegen-units = 1` is deliberate and `wasm-opt` is single-threaded. A dev
  // build is seconds. For "does this behave correctly", which is what almost
  // every run here is asking, the optimised bundle buys nothing but waiting.
  //
  // The exception is not negotiable. The measured specs assert on real
  // numbers: `engine-limits` gates `fps > 20` across a token sweep, and
  // `canvas-authoring` holds map import inside SC-007's 30 seconds. Those were
  // *already* measured against an unoptimised build once — the map-import
  // budget was failing at 36.1s until the image codecs were given
  // `opt-level = 3` in the dev profile, which took it to 21.9s without the
  // product changing at all. A dev-built number there describes rustc, not the
  // engine.
  //
  // So: the lane that measures gets the build worth measuring, and everything
  // else gets its result minutes sooner. `ENGINE_PROFILE` still overrides both.
  const measuredWillRun = measuredSpecsSelected(args).length > 0;
  const profile = engineProfile(measuredWillRun ? "release" : "dev");
  if (measuredWillRun && profile === "release") {
    log(
      "e2e",
      "This run includes specs that measure the engine, so it is built release.",
    );
  }
  await ensurePdfBuild({});
  await ensureEngineBuild({
    profile,
    noOpt: skipWasmOpt() && !measuredWillRun,
  });

  // Once, before any shard starts. N concurrent `cargo run`s would serialise
  // on the target-directory lock anyway, and the first shard would look hung.
  await runCommand("cargo build -p thunderforge", {
    name: "build server",
    prefix: "e2e",
  });

  await provisionTemplate();

  for (let index = 0; index < total; index += 1) {
    const { shard, reason } = await startStack(index);
    if (!shard) {
      // Every sharded stack is needed before any shard can run, because the
      // partition below assigns files to all of them. Say which one failed
      // and why, in the summary as well as the log, then stop.
      run.results.push(stackFailure(index, "parallel", reason));
      return finish();
    }
    log(
      "e2e",
      `Shard ${index} up on :${shard.webPort} (db ${shard.database}).`,
    );
  }
  const shards = run.shards;

  // `--only` takes a comma-separated list, so a triage run can name exactly
  // the handful of specs under suspicion rather than a prefix that drags in
  // their neighbours.
  const onlyPatterns = args.only
    ?.split(",")
    .map((p) => p.trim())
    .filter(Boolean);
  const specs = allSpecFiles(args.suite).filter(
    (file) => !onlyPatterns || onlyPatterns.some((p) => file.includes(p)),
  );
  if (specs.length === 0) {
    throw new Error(`--only=${args.only} matched no spec files`);
  }
  // Three lanes, and they are a partition of `specs`.
  //
  // The first-run lane is separated before the other two rather than after,
  // because its specs must never reach a seeded stack: an `instance-setup`
  // spec run against shard 0 would find setup already complete and fail for a
  // reason that has nothing to do with what it tests. `--all` shards the
  // *measured* specs; it deliberately does not move these, because the
  // distinction here is which database they need, not how they are timed.
  //
  // A playtest run has one lane of its own and none of these: its scenarios
  // build their own worlds, and none of them is measured.
  const playtest = args.suite === "playtest";
  const playtestSpecs = playtest ? specs : [];
  const firstRunSpecs = playtest ? [] : specs.filter(isFirstRunSpec);
  const githubAppsSpecs = playtest ? [] : specs.filter(isGithubAppsSpec);
  const rest = playtest
    ? []
    : specs.filter((file) => !isFirstRunSpec(file) && !isGithubAppsSpec(file));
  const parallelSpecs = args.all
    ? rest
    : rest.filter((file) => !isPerfSpec(file));
  const serialSpecs = args.all ? [] : rest.filter(isPerfSpec);

  // `--only` naming nothing but measured specs is almost always a mistake: the
  // sharded lane gets no files, and the whole run collapses to the serial lane
  // on one shard, which is not what someone asking for shards wanted. Say so,
  // and name the flag that does what they meant.
  if (
    parallelSpecs.length === 0 &&
    serialSpecs.length === 0 &&
    firstRunSpecs.length > 0
  ) {
    log(
      "e2e",
      `${firstRunSpecs.length} first-run spec file(s); the sharded lane has nothing to do.`,
    );
  }
  if (parallelSpecs.length === 0 && serialSpecs.length > 0 && onlyPatterns) {
    log(
      "e2e",
      `--only=${args.only} matched only measured specs (${serialSpecs.join(", ")}).` +
        " Pass --all to shard them, or expect the serial lane alone.",
    );
  }
  log(
    "e2e",
    `${parallelSpecs.length} spec files sharded, ${serialSpecs.length} measured serially, ` +
      `${githubAppsSpecs.length} on the GitHub-applications stack.`,
  );

  const durations = readDurations();
  run.durations = durations;
  const bins = partitionByDuration(parallelSpecs, total, durations);
  for (const [index, bin] of bins.entries()) {
    log(
      "e2e",
      `  shard ${index}: ${bin.files.length} files, ~${(bin.cost / 60).toFixed(1)} min estimated.`,
    );
  }

  run.started = Date.now();
  run.load.setPhase("tests");
  run.results.push(
    ...(await Promise.all(
      shards.map((shard) => runShard(shard, bins[shard.index].files)),
    )),
  );

  if (playtestSpecs.length > 0) {
    log(
      "e2e",
      `Running ${playtestSpecs.length} playtest(s); the report goes to apps/web/${SUITES.playtest.report}.`,
    );
    run.results.push(
      await runShard(shards[0], playtestSpecs, "playtest", SUITES.playtest),
    );
  }

  // The measured specs, alone, on the first shard's stack. Sequential by
  // construction: this is the lane whose numbers are only meaningful when
  // nothing else is competing for the GPU.
  if (serialSpecs.length > 0) {
    log("e2e", "Sharded lane done; running the measured specs alone.");
    run.results.push(await runShard(shards[0], serialSpecs, "serial"));
  }

  // First run, on a stack of its own, after everything else.
  //
  // It needs its own stack because it *completes setup* — it writes the
  // instance's identity, its operator and its first administrator — and those
  // are instance-wide. Sharing a database with any other lane would leave that
  // lane's later tests running against an instance somebody else just
  // configured, which is exactly the class of cross-test contamination the
  // per-shard database exists to prevent.
  if (firstRunSpecs.length > 0) {
    log("e2e", "Running the first-run lane on an unseeded stack.");
    await provisionFirstRunTemplate();
    const { shard, reason } = await startStack(total, { firstRun: true });
    if (!shard) {
      run.results.push(stackFailure(total, "first-run", reason));
    } else {
      log(
        "e2e",
        `First-run stack up on :${shard.webPort} (db ${shard.database}, unseeded).`,
      );
      run.results.push(await runShard(shard, firstRunSpecs, "first-run"));
    }
  }

  // The GitHub-applications lane, on its own stack for the reason
  // `isGithubAppsSpec` gives. Index `total + 1`, one past the first-run stack.
  if (githubAppsSpecs.length > 0) {
    log("e2e", "Running the GitHub-applications lane on its own stack.");
    const { shard, reason } = await startStack(total + 1, {
      githubApps: true,
    });
    if (!shard) {
      run.results.push(stackFailure(total + 1, "github-apps", reason));
    } else {
      run.results.push(await runShard(shard, githubAppsSpecs, "github-apps"));
    }
  }

  return finish();
}

/**
 * Everything this run has done so far, kept outside `main` so that a crash, a
 * signal or a stack that would not start still ends in a summary. On
 * 2026-09-14 a `docker run` error escaped `startMailpit`, the harness died on
 * the stack trace, and the lane it was starting never appeared in any summary.
 */
const run = {
  args: null,
  results: [],
  shards: [],
  durations: null,
  started: Date.now(),
  finishing: false,
  startedAt: new Date().toISOString(),
  load: null,
};

/**
 * One line about the machine before anything is built, so a run started on
 * top of someone else's build says so at the top of the log as well as in the
 * summary at the bottom.
 */
function logLoadAtStart(load) {
  const { load1, load5 } = load.atStart;
  log(
    "e2e",
    `Load at start: ${load1} (5 min ${load5}) on ${load.cpus} CPUs, ` +
      `${load.atStart.freeMemGiB} GiB free.`,
  );
  for (const f of load.foreign) {
    log(
      "e2e",
      `  Also running, not started by this run: ${f.kind} in ${f.cwd}.`,
      process.stderr,
    );
  }
}

/** The first line of an error, for a one-line reason. */
function firstLine(error) {
  return String(error?.message ?? error).split("\n")[0];
}

/**
 * Start a stack and say why not, instead of throwing or returning a bare null.
 *
 * A thrown error (docker refused a container, `psql` failed to clone) and a
 * service that never became ready both become a reason the summary can print.
 */
async function startStack(index, options = {}) {
  try {
    const shard = await startShard(index, options);
    if (shard) {
      run.shards.push(shard);
      return { shard };
    }
    return {
      reason:
        "stack failed to start: a service never became ready (see the log above)",
    };
  } catch (error) {
    log("e2e", String(error?.stack ?? error), process.stderr);
    return { reason: `stack failed to start: ${firstLine(error)}` };
  }
}

function stackFailure(index, label, reason) {
  log("e2e", `The ${label} stack (${index}): ${reason}`, process.stderr);
  return { index, label, code: 1, failed: true, reasons: [reason] };
}

/**
 * Summarise, tear down, release, exit — once, from whichever path got here.
 *
 * `exitCode` forces the code (a signal); otherwise it is 1 when any lane
 * failed or nothing ran at all.
 */
async function finish(exitCode = null) {
  if (run.finishing) return;
  run.finishing = true;

  if (run.durations) {
    try {
      recordDurations(
        run.shards.map((shard) => join(SHARD_DIR, `shard-${shard.index}`)),
        run.durations,
        { baseline: Boolean(run.args?.recordDurations) },
      );
    } catch (error) {
      log(
        "e2e",
        `Could not record durations: ${firstLine(error)}`,
        process.stderr,
      );
    }
  }

  const failed = run.results.filter((result) => result.failed);
  const minutes = ((Date.now() - run.started) / 60_000).toFixed(1);
  log("e2e", `Finished in ${minutes} minutes.`);
  if (run.results.length === 0) {
    log("e2e", "  No lane ran.", process.stderr);
  }
  for (const result of run.results) {
    const verdict = result.failed
      ? `FAILED (${result.reasons.join(", ")})`
      : result.skipped
        ? "no files"
        : "passed";
    log("e2e", `  ${result.label} lane, shard ${result.index}: ${verdict}`);
  }
  if (failed.length > 0) {
    log(
      "e2e",
      `${failed.length} lane run(s) failed: ` +
        failed.map((r) => `${r.label} lane, shard ${r.index}`).join("; "),
      process.stderr,
    );
  }

  // Before teardown, so the load samples describe the run rather than the
  // teardown; printed after it, so the digest is the last thing in the log.
  let digest = [];
  if (run.load) {
    try {
      const summary = writeRunSummary({
        root: ROOT_DIR,
        results: run.results,
        load: run.load.stop(),
        args: process.argv.slice(2),
        startedAt: run.startedAt,
      });
      digest = digestLines(summary, ROOT_DIR);
    } catch (error) {
      log(
        "e2e",
        `Could not write the run summary: ${String(error?.stack ?? error)}`,
        process.stderr,
      );
    }
  }

  await terminateChildren("SIGTERM");
  // Containers are not children of this process, so `terminateChildren` does
  // not reach them. Removed even under `--keep`, which preserves *databases*
  // for inspection; a held port is nobody's idea of a useful artefact.
  stopMailpit();
  if (!run.args?.keep) {
    for (const database of clonedDatabases) {
      try {
        psql("postgres", `DROP DATABASE IF EXISTS ${database} WITH (FORCE);`);
      } catch (error) {
        log(
          "e2e",
          `Could not drop ${database}: ${firstLine(error)}`,
          process.stderr,
        );
      }
    }
  }

  for (const line of digest) log("e2e", line);
  releaseRunLock(ROOT_DIR);
  process.exit(
    exitCode ?? (failed.length > 0 || run.results.length === 0 ? 1 : 0),
  );
}

/**
 * Ctrl-C, `kill`, a closed terminal: summarise what ran, take the stacks down
 * and release the lock.
 *
 * Without this a signal killed the runner outright and left everything it
 * started behind — the servers are spawned detached, in process groups of
 * their own, so the terminal's SIGINT never reached them — and the lock stayed
 * behind to refuse the next commit. `exit` covers the paths that call
 * `process.exit` directly. A `kill -9` reaches none of this; the lock then
 * names a dead pid, which `run-lock.mjs` treats as stale.
 */
function releaseOnSignals() {
  process.once("exit", () => releaseRunLock(ROOT_DIR));
  for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
    process.once(signal, () => {
      log("e2e", `Received ${signal}; stopping.`, process.stderr);
      run.results.push({
        index: "-",
        label: "harness",
        code: 130,
        failed: true,
        reasons: [`interrupted by ${signal}`],
      });
      void finish(130);
    });
  }
}

main().catch((error) => {
  log("e2e", String(error?.stack ?? error), process.stderr);
  run.results.push({
    index: "-",
    label: "harness",
    code: 1,
    failed: true,
    reasons: [`crashed: ${firstLine(error)}`],
  });
  void finish(1);
});
