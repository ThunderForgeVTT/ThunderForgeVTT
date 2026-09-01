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
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { join, relative } from "node:path";

import {
  ROOT_DIR,
  ensureEngineBuild,
  engineProfile,
  log,
  runCommand,
  spawnManaged,
  terminateChildren,
} from "./shared.mjs";

/** Away from 5173/30000 on purpose, so a `pnpm dev` can stay up while this runs. */
const WEB_PORT_BASE = 5200;
const BACKEND_PORT_BASE = 30100;

const POSTGRES_CONTAINER =
  process.env.THUNDERFORGE_POSTGRES_CONTAINER ?? "thunderforge-postgres";
const DB_USER = process.env.THUNDERFORGE_DB_USER ?? "postgres";
const TEMPLATE_DB = "thunderforge_e2e_template";
const SHARD_DIR = join(ROOT_DIR, ".e2e-shards");
/** Measured seconds per spec file, so each run balances better than the last. */
const DURATIONS_PATH = join(ROOT_DIR, ".e2e-shards-durations.json");
const LOCK_PATH = join(ROOT_DIR, ".e2e-shards.lock");

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
  // These two were added from evidence rather than by reading assertions:
  // both failed in a four-shard run and both pass alone, `status-systems` in
  // about six seconds per ruleset and `world-cache-isolated` in 1.4 minutes.
  // Neither asserts on a duration, so the earlier pass over the suite missed
  // them — what they actually need is an engine that can report inside a 60s
  // predicate, which it cannot while three other shards compete for the GPU.
  "status-systems",
  "world-cache-isolated",
];

/** Every spec file, relative to `apps/web`, including `e2e/torture`. */
function allSpecFiles() {
  const root = join(ROOT_DIR, "apps/web/e2e");
  const found = [];
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.name.endsWith(".spec.ts")) {
        found.push(relative(join(ROOT_DIR, "apps/web"), full));
      }
    }
  };
  walk(root);
  return found.sort();
}

function isPerfSpec(file) {
  return PERF_LANE_SPECS.some((name) => file.endsWith(`/${name}.spec.ts`));
}

/** Seconds per spec file from the last run, or `{}` on the first one. */
function readDurations() {
  try {
    return JSON.parse(readFileSync(DURATIONS_PATH, "utf-8"));
  } catch {
    return {};
  }
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
  const known = files.map((f) => durations[f]).filter((d) => typeof d === "number");
  const fallback = known.length > 0 ? known.reduce((a, b) => a + b, 0) / known.length : 1;
  const weighted = files
    .map((file) => ({ file, cost: durations[file] ?? fallback }))
    .sort((a, b) => b.cost - a.cost);

  const bins = Array.from({ length: shardCount }, () => ({ cost: 0, files: [] }));
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
 */
function recordDurations(shardDirs, previous) {
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
            totals[`e2e/${path}`] = (totals[`e2e/${path}`] ?? 0) + (result.duration ?? 0) / 1000;
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
  writeFileSync(DURATIONS_PATH, JSON.stringify(totals, null, 2));
}

/**
 * Refuse to start on top of a run that is already going.
 *
 * This script begins by deleting the shard directory and dropping the template
 * database, so a second invocation does not merely contend — it removes the
 * demo sessions, JSON reports and databases the live run is in the middle of
 * using, and the failures surface inside whatever tests happened to be running
 * as bugs of their own.
 *
 * A stale lock is reclaimed rather than fatal: `process.kill(pid, 0)` throws if
 * nothing is there, and a run killed with Ctrl-C leaves one behind every time.
 */
function acquireLock() {
  try {
    const previous = Number(readFileSync(LOCK_PATH, "utf-8").trim());
    if (Number.isInteger(previous) && previous > 0) {
      try {
        process.kill(previous, 0);
        throw new Error(
          `another e2e-parallel run is active (pid ${previous}). ` +
            `Wait for it, or remove ${LOCK_PATH} if you are sure it is gone.`,
        );
      } catch (error) {
        // ESRCH: the pid is gone, so the lock is stale and ours to take.
        if (error instanceof Error && !/^another e2e-parallel/.test(error.message)) {
          log("e2e", `Reclaiming a stale lock from pid ${previous}.`);
        } else {
          throw error;
        }
      }
    }
  } catch (error) {
    if (error instanceof Error && /^another e2e-parallel/.test(error.message)) {
      throw error;
    }
    // No lock file at all, which is the ordinary case.
  }
  writeFileSync(LOCK_PATH, String(process.pid));
}

function releaseLock() {
  try {
    rmSync(LOCK_PATH, { force: true });
  } catch {
    // Nothing to release.
  }
}

function psql(database, sql) {
  return execFileSync(
    "docker",
    ["exec", "-i", POSTGRES_CONTAINER, "psql", "-U", DB_USER, "-d", database, "-v", "ON_ERROR_STOP=1", "-q"],
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

function cloneShardDatabase(index) {
  const name = shardDbName(index);
  psql("postgres", `DROP DATABASE IF EXISTS ${name} WITH (FORCE);`);
  psql("postgres", `CREATE DATABASE ${name} TEMPLATE ${TEMPLATE_DB};`);
  return name;
}

/**
 * Give a shard the installed game systems, the way a dev stack has them.
 *
 * `config/mod.rs` resolves `systems_dir` as `<data path>/packs/systems`, and
 * the server reads `<systems_dir>/<system id>/system.json` to answer for a
 * system's attributes and status resources. A shard pointed at a fresh data
 * path therefore has *no systems installed* — `updateWorldGameSystem`
 * succeeds, a token is created, and `tokenAttributes` then returns nothing for
 * it, which surfaces as "genie must resolve attributes for its token" rather
 * than as anything about a missing directory.
 *
 * Symlinks rather than copies, which is exactly what `data/packs/systems`
 * already is on a dev machine: the eight shipping packs are 110MB, and copying
 * them per shard would cost more than the parallelism saves.
 */
function linkSystemPacks(dataPath) {
  const source = join(ROOT_DIR, "packs/systems");
  const target = join(dataPath, "packs", "systems");
  mkdirSync(target, { recursive: true });
  for (const entry of readdirSync(source, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    symlinkSync(join(source, entry.name), join(target, entry.name), "dir");
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

/** Starts one shard's backend and frontend, and resolves once both answer. */
async function startShard(index) {
  const database = cloneShardDatabase(index);
  const backendPort = BACKEND_PORT_BASE + index;
  const webPort = WEB_PORT_BASE + index;
  const dataPath = join(SHARD_DIR, `shard-${index}`, "data");
  mkdirSync(dataPath, { recursive: true });
  linkSystemPacks(dataPath);

  const shared = {
    DATABASE_URL: `postgres://${DB_USER}:password@localhost:5432/${database}`,
    RUSTFS_BUCKET: `tf-e2e-${index}`,
    THUNDERFORGE_DATA_PATH: dataPath,
    // The single highest-leverage variable in the whole harness: the per-IP
    // auth limiter accounted for 18 of the 42 failures the last full sweep
    // started from, and every shard registers users from the same IP.
    THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT: "1",
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
  };

  spawnManaged(`./target/debug/thunderforge --port ${backendPort}`, {
    cwd: ROOT_DIR,
    prefix: `be${index}`,
    env: shared,
  });
  // `/api/readyz`, not `/readyz`: every backend route is nested under `/api`
  // (`main.rs`), and readiness rather than liveness because it is the one that
  // says the database is reachable — which for a freshly cloned shard database
  // is the fact actually in question.
  if (!(await waitForUrl(`http://127.0.0.1:${backendPort}/api/readyz`, `backend ${index}`))) {
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
  if (!(await waitForUrl(`http://127.0.0.1:${webPort}/`, `frontend ${index}`))) {
    return null;
  }

  return { index, database, webPort, backendPort };
}

/** Runs one Playwright shard against an already-started stack. */
function runShard(shard, files, label = "parallel") {
  const demoDir = join(SHARD_DIR, `shard-${shard.index}`, "demo");
  mkdirSync(demoDir, { recursive: true });

  // File paths rather than `--grep`: Playwright matches `--grep` against the
  // test *title*, so selecting by filename that way depends on titles happening
  // to mention their file. Positional arguments are matched against the path,
  // which is what is actually meant here — and naming the files makes the two
  // lanes provably a partition, since they come from one list.
  //
  // No `--shard`: the split is done here, by measured duration
  // (`partitionByDuration`), because Playwright's own divides by test count and
  // cannot know that one file is a quarter of the suite.
  const command =
    `pnpm exec playwright test ${files.join(" ")}` +
    ` --workers=1 --reporter=list,json --output=test-results/shard-${shard.index}`;

  const child = spawnManaged(command, {
    cwd: join(ROOT_DIR, "apps/web"),
    prefix: `sh${shard.index}`,
    env: {
      PLAYWRIGHT_BASE_URL: `http://127.0.0.1:${shard.webPort}`,
      THUNDERFORGE_E2E_EXTERNAL_STACK: "1",
      THUNDERFORGE_E2E_DEMO_DIR: demoDir,
      THUNDERFORGE_DB_NAME: shard.database,
      // Playwright names the JSON report by env var, not by flag. Labelled
      // because shard 0 runs twice — its share of the parallel lane, then the
      // measured lane alone — and a single name meant the second run erased
      // the first. That silently dropped `token-authoring` from the recorded
      // durations, which is the file the whole partition is built around.
      PLAYWRIGHT_JSON_OUTPUT_NAME: join(
        SHARD_DIR,
        `shard-${shard.index}`,
        `results-${label}.json`,
      ),
    },
  });

  return new Promise((resolve) => {
    child.once("close", (code) => resolve({ index: shard.index, code: code ?? 1 }));
  });
}

async function main() {
  // Parsed here rather than through `shared.mjs`'s `parseArgs`, which is a
  // fixed-shape parser for the dev/build scripts' three flags and rejects
  // anything else by design.
  const args = { shards: 4, all: false, keep: false, only: null };
  for (const argv of process.argv.slice(2)) {
    const shardMatch = /^--shards=(\d+)$/.exec(argv);
    const onlyMatch = /^--only=(.+)$/.exec(argv);
    if (shardMatch) args.shards = Number(shardMatch[1]);
    // A substring of the spec path, for exercising the harness itself without
    // waiting out the suite it exists to speed up.
    else if (onlyMatch) args.only = onlyMatch[1];
    else if (argv === "--all") args.all = true;
    else if (argv === "--keep") args.keep = true;
    else throw new Error(`Unknown argument: ${argv}`);
  }
  const total = args.shards;
  if (!Number.isInteger(total) || total < 1) {
    throw new Error(`--shards must be a positive integer, got ${total}`);
  }

  acquireLock();
  log("e2e", `Preparing ${total} shard${total === 1 ? "" : "s"}.`);
  rmSync(SHARD_DIR, { recursive: true, force: true });
  mkdirSync(SHARD_DIR, { recursive: true });

  // Release, matching what the single-stack e2e path uses: a 57MB unoptimised
  // code section compiled by the browser on every page load costs far more,
  // across a whole suite, than the one-off build does.
  await ensureEngineBuild({ profile: engineProfile("release") });

  // Once, before any shard starts. N concurrent `cargo run`s would serialise
  // on the target-directory lock anyway, and the first shard would look hung.
  await runCommand("cargo build -p thunderforge", {
    name: "build server",
    prefix: "e2e",
  });

  await provisionTemplate();

  const shards = [];
  for (let index = 0; index < total; index += 1) {
    const shard = await startShard(index);
    if (!shard) {
      log("e2e", `Shard ${index} failed to start; aborting.`, process.stderr);
      await terminateChildren("SIGTERM");
      process.exit(1);
    }
    shards.push(shard);
    log("e2e", `Shard ${index} up on :${shard.webPort} (db ${shard.database}).`);
  }

  // `--only` takes a comma-separated list, so a triage run can name exactly
  // the handful of specs under suspicion rather than a prefix that drags in
  // their neighbours.
  const onlyPatterns = args.only?.split(",").map((p) => p.trim()).filter(Boolean);
  const specs = allSpecFiles().filter(
    (file) => !onlyPatterns || onlyPatterns.some((p) => file.includes(p)),
  );
  if (specs.length === 0) {
    throw new Error(`--only=${args.only} matched no spec files`);
  }
  const parallelSpecs = args.all ? specs : specs.filter((file) => !isPerfSpec(file));
  const serialSpecs = args.all ? [] : specs.filter(isPerfSpec);
  log("e2e", `${parallelSpecs.length} spec files sharded, ${serialSpecs.length} measured serially.`);

  const durations = readDurations();
  const bins = partitionByDuration(parallelSpecs, total, durations);
  for (const [index, bin] of bins.entries()) {
    log("e2e", `  shard ${index}: ${bin.files.length} files, ~${(bin.cost / 60).toFixed(1)} min estimated.`);
  }

  const started = Date.now();
  const results = await Promise.all(
    shards.map((shard) => runShard(shard, bins[shard.index].files)),
  );

  // The measured specs, alone, on the first shard's stack. Sequential by
  // construction: this is the lane whose numbers are only meaningful when
  // nothing else is competing for the GPU.
  if (serialSpecs.length > 0) {
    log("e2e", "Sharded lane done; running the measured specs alone.");
    results.push(await runShard(shards[0], serialSpecs, "serial"));
  }

  recordDurations(
    shards.map((shard) => join(SHARD_DIR, `shard-${shard.index}`)),
    durations,
  );

  const minutes = ((Date.now() - started) / 60_000).toFixed(1);
  const failed = results.filter((result) => result.code !== 0);
  log("e2e", `Finished in ${minutes} minutes.`);
  for (const result of results) {
    log("e2e", `  shard ${result.index}: ${result.code === 0 ? "passed" : `FAILED (${result.code})`}`);
  }

  await terminateChildren("SIGTERM");
  if (!args.keep) {
    for (const shard of shards) {
      psql("postgres", `DROP DATABASE IF EXISTS ${shard.database} WITH (FORCE);`);
    }
  }

  releaseLock();
  process.exit(failed.length > 0 ? 1 : 0);
}

main().catch((error) => {
  log("e2e", String(error?.stack ?? error), process.stderr);
  releaseLock();
  void terminateChildren("SIGTERM").finally(() => process.exit(1));
});
