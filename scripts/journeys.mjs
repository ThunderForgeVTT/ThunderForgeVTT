#!/usr/bin/env node
/**
 * Run the journeys against an instance of their own.
 *
 *   pnpm journeys                            # every journey
 *   pnpm journeys --only=operator-pauses     # journeys whose path contains it
 *   pnpm journeys --keep                     # leave the containers up after
 *
 * # What a journey is, and why it needs its own instance
 *
 * A journey goes through the product the way a person does: UI only, with
 * GraphQL used to *check* what the server holds and never to take a shortcut
 * (spec 051, "Journeys"). Some of what a person does there is instance-wide —
 * an operator enrols a second factor, pauses a world, lifts it — and none of
 * that should land in the dev database or in an `e2e-parallel` shard's database
 * while that shard is in the middle of its own suite.
 *
 * So this follows `scripts/torture.mjs` rather than `scripts/e2e-parallel.mjs`:
 * Postgres, RustFS and Mailpit come up on tmpfs in a compose project named for
 * this run, on ports the kernel says are free, and teardown is stopping the
 * containers. The app itself runs natively, as it does in both of those.
 *
 * It deliberately takes **no** `e2e-parallel` lock and touches none of its
 * state (`.e2e-shards`, the template databases, the dev Postgres container),
 * so a journey run and a sharded suite run can overlap without either knowing.
 * They will compete for CPU and GPU, which is its own problem, but they will
 * not corrupt each other.
 *
 * # Teardown is not optional
 *
 * Every exit path — success, a failed journey, a stack that never came up, an
 * exception in this script, Ctrl-C, SIGTERM — stops the backend and Vite and
 * runs `docker compose down`. A leftover backend holds a port and a leftover
 * container holds memory, and neither says where it came from.
 */
import { execFileSync, spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  openSync,
  readdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
} from "node:fs";
import { createServer } from "node:net";
import { join, relative } from "node:path";
import process from "node:process";

import {
  ROOT_DIR,
  engineProfile,
  ensureEngineBuild,
  ensurePdfBuild,
  log,
  runCommand,
  skipWasmOpt,
  terminateChildren,
} from "./shared.mjs";

const COMPOSE_FILE = join(ROOT_DIR, "compose.journeys.yml");
const JOURNEY_DIR = join(ROOT_DIR, "apps/web/e2e/journeys");
const JOURNEY_SUFFIX = ".journey.spec.ts";

// ---------------------------------------------------------------------------
// Arguments
// ---------------------------------------------------------------------------

// The same two flags `e2e-parallel.mjs` takes, with the same meaning, so a
// person switching between the lanes does not have to learn a second spelling.
const args = { only: null, keep: false };
for (const argv of process.argv.slice(2)) {
  const onlyMatch = /^--only=(.+)$/.exec(argv);
  if (onlyMatch) args.only = onlyMatch[1];
  // Keeps the *containers* — the database, the bucket, the mailbox — for
  // inspection, as `e2e-parallel --keep` keeps its databases. The backend and
  // Vite are stopped regardless: a process holding a port is nobody's idea of
  // a useful artefact, and the database is where the evidence is.
  else if (argv === "--keep") args.keep = true;
  else {
    console.error(`[journeys] unknown argument: ${argv}`);
    process.exit(2);
  }
}

// ---------------------------------------------------------------------------
// Identity of this run
// ---------------------------------------------------------------------------

const runId = randomBytes(4).toString("hex");
/** Unique per run, so two runs overlap without sharing a container. */
const project = `tf-journeys-${runId}`;
const runDir = join(ROOT_DIR, ".journeys", runId);
/** Per run, so an overlapping run cannot overwrite this one's report. */
const resultsDir = join(ROOT_DIR, "apps/web/journeys-results", runId);

/**
 * A port nothing is listening on, from the kernel.
 *
 * Asked for rather than derived from the run id, as torture does, because a
 * journey run is meant to overlap an `e2e-parallel` run whose fixed port
 * ranges (5200+, 30100+, 31025+, 38025+ ...) a derived number could land in.
 * The port is released before it is used, so there is a window in which
 * something else could take it; a backend that then fails to bind exits, and
 * the readiness wait below races that exit rather than trusting a `fetch`.
 */
function freePort() {
  return new Promise((resolve, reject) => {
    const probe = createServer();
    probe.once("error", reject);
    probe.listen(0, "127.0.0.1", () => {
      const { port } = probe.address();
      probe.close(() => resolve(port));
    });
  });
}

// ---------------------------------------------------------------------------
// Processes and teardown
// ---------------------------------------------------------------------------

/** Long-running children this script started: the backend and Vite. */
const services = [];
let composeStarted = false;
let tearingDown = null;

/**
 * Start a long-running process with its output in a log file.
 *
 * Not `spawnManaged` from `shared.mjs`, for one reason: that prints every line
 * to this terminal, and a debug backend's log would bury the Playwright list a
 * person is here to read. The files are kept under `.journeys/<run>/` and the
 * failure message says where.
 *
 * Detached, so it leads a process group of its own and `pnpm`'s children
 * (Vite under `pnpm run dev`) die with it when the group is signalled.
 */
function startService(name, command, commandArgs, { cwd, env }) {
  const logPath = join(runDir, `${name}.log`);
  const out = openSync(logPath, "a");
  const child = spawn(command, commandArgs, {
    cwd,
    env: { ...process.env, ...env },
    detached: true,
    stdio: ["ignore", out, out],
  });
  services.push({ name, child });
  log(
    "journeys",
    `${name} started (pid ${child.pid}), log: ${relative(ROOT_DIR, logPath)}`,
  );
  return { child, logPath };
}

function signalGroup(child, signal) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  try {
    process.kill(-child.pid, signal);
  } catch {
    try {
      child.kill(signal);
    } catch {
      // Already gone.
    }
  }
}

async function stopServices() {
  const alive = services.filter(
    ({ child }) => child.exitCode === null && child.signalCode === null,
  );
  if (alive.length === 0) return;
  log("journeys", `stopping ${alive.map((s) => s.name).join(", ")}`);
  const closed = Promise.all(
    alive.map(
      ({ child }) =>
        new Promise((resolve) => {
          if (child.exitCode !== null || child.signalCode !== null) resolve();
          else child.once("exit", resolve);
        }),
    ),
  );
  for (const { child } of alive) signalGroup(child, "SIGTERM");
  const timedOut = await Promise.race([
    closed.then(() => false),
    new Promise((resolve) => setTimeout(() => resolve(true), 5_000)),
  ]);
  if (timedOut) {
    for (const { child } of alive) signalGroup(child, "SIGKILL");
    await Promise.race([
      closed,
      new Promise((resolve) => setTimeout(resolve, 2_000)),
    ]);
  }
}

function composeArgs(...rest) {
  return ["compose", "-f", COMPOSE_FILE, "-p", project, ...rest];
}

/**
 * Stop everything this run started. Idempotent: the signal handlers and the
 * ordinary exit path can both reach it, and the second caller waits on the
 * first rather than tearing down twice.
 */
function teardown() {
  if (tearingDown) return tearingDown;
  tearingDown = (async () => {
    await stopServices();
    // Builds run through `spawnManaged`; a Ctrl-C during one leaves them here.
    await terminateChildren("SIGTERM");
    if (composeStarted) {
      if (args.keep) {
        log(
          "journeys",
          `--keep: containers left up. Remove them with:\n` +
            `  docker compose -f compose.journeys.yml -p ${project} down -v`,
        );
      } else {
        log("journeys", `tearing down ${project}`);
        try {
          // `-v` as well as `down`: tmpfs leaves nothing behind, but if
          // someone later swaps tmpfs for a volume this keeps the promise.
          execFileSync(
            "docker",
            composeArgs("down", "-v", "--remove-orphans", "--timeout", "5"),
            { stdio: "inherit", env: composeEnv },
          );
        } catch {
          log(
            "journeys",
            `teardown failed — clean up by hand:\n` +
              `  docker compose -f compose.journeys.yml -p ${project} down -v`,
            process.stderr,
          );
        }
      }
    }
    // The data path is symlinks to the shipped packs and whatever the backend
    // wrote while a journey ran; the logs beside it are only worth keeping
    // when someone asked to keep things.
    if (!args.keep) {
      rmSync(join(runDir, "data"), { recursive: true, force: true });
    }
  })();
  return tearingDown;
}

// Ctrl-C and SIGTERM tear down too, and then exit with the conventional code.
// Registered before anything is started, so there is no window in which a
// signal would kill this script and leave its children behind.
for (const [signal, code] of [
  ["SIGINT", 130],
  ["SIGTERM", 143],
  ["SIGHUP", 129],
]) {
  process.on(signal, () => {
    log("journeys", `${signal} received`, process.stderr);
    void teardown().finally(() => process.exit(code));
  });
}

// ---------------------------------------------------------------------------
// Readiness
// ---------------------------------------------------------------------------

async function waitForUrl(
  url,
  name,
  timeoutMs,
  child = null,
  { anyStatus = false } = {},
) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (child && (child.exitCode !== null || child.signalCode !== null)) {
      throw new Error(
        `${name} exited (${child.exitCode ?? child.signalCode}) before it answered on ${url}`,
      );
    }
    try {
      const response = await fetch(url);
      if (anyStatus || response.ok) return;
    } catch {
      // Not up yet.
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error(`${name} never became ready at ${url}`);
}

/**
 * Fail loudly if a container died on startup. `up` succeeds for a container
 * that starts and immediately exits, which is exactly what RustFS did for the
 * whole history of the torture harness (see `compose.torture.yml`).
 */
function assertContainerRunning(service) {
  const state = execFileSync(
    "docker",
    composeArgs("ps", "-a", "--format", "{{.State}}", service),
    { env: composeEnv, encoding: "utf-8" },
  ).trim();
  if (!state.startsWith("running")) {
    try {
      execFileSync("docker", composeArgs("logs", service), {
        env: composeEnv,
        stdio: "inherit",
      });
    } catch {
      // The logs are a courtesy; the failure below is the point.
    }
    throw new Error(`${service} is "${state}", not running`);
  }
}

// ---------------------------------------------------------------------------
// The instance
// ---------------------------------------------------------------------------

/**
 * Give the instance the installed packs, the way a dev stack has them.
 *
 * Copied from `linkPacks` in `scripts/e2e-parallel.mjs`, whose comment is the
 * full story: a backend pointed at a fresh data path has *nothing installed*,
 * and a world created on it resolves no token attributes and no interface
 * pack, failing far from the cause. That script runs `main()` on import, so it
 * cannot be imported from; if this grows a third copy, move it to `shared.mjs`.
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

function journeyFiles() {
  if (!existsSync(JOURNEY_DIR)) return [];
  const found = [];
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.name.endsWith(JOURNEY_SUFFIX)) {
        found.push(relative(join(ROOT_DIR, "apps/web"), full));
      }
    }
  };
  walk(JOURNEY_DIR);
  return found.sort();
}

/**
 * The verdict, from what Playwright *reported* as well as how it exited — the
 * same two channels `judgeLane` in `e2e-parallel.mjs` reads, for the reason it
 * gives: an exit code was once lost and a red run reported green.
 */
function judge(code, reportPath) {
  const reasons = [];
  if (code !== 0) reasons.push(`exit ${code}`);
  let report = null;
  try {
    report = JSON.parse(readFileSync(reportPath, "utf-8"));
  } catch {
    reasons.push("no JSON report");
  }
  if (report) {
    const { expected = 0, unexpected = 0, skipped = 0 } = report.stats ?? {};
    if (unexpected > 0) reasons.push(`${unexpected} journey(s) failed`);
    if ((report.errors?.length ?? 0) > 0) {
      reasons.push(`${report.errors.length} runner error(s)`);
    }
    if (expected + unexpected === 0 && skipped === 0) {
      reasons.push("no journeys ran");
    }
    return { reasons, stats: { expected, unexpected, skipped } };
  }
  return { reasons, stats: null };
}

let composeEnv = { ...process.env };

async function main() {
  const selected = journeyFiles().filter(
    (file) => !args.only || file.includes(args.only),
  );
  if (selected.length === 0) {
    throw new Error(
      args.only
        ? `--only=${args.only} matched no *${JOURNEY_SUFFIX} under apps/web/e2e/journeys`
        : `no *${JOURNEY_SUFFIX} under apps/web/e2e/journeys`,
    );
  }

  const [pgPort, rustfsPort, smtpPort, mailApiPort, backendPort, webPort] =
    await Promise.all(Array.from({ length: 6 }, freePort));
  composeEnv = {
    ...process.env,
    JOURNEYS_PG_PORT: String(pgPort),
    JOURNEYS_RUSTFS_PORT: String(rustfsPort),
    JOURNEYS_MAILPIT_SMTP_PORT: String(smtpPort),
    JOURNEYS_MAILPIT_API_PORT: String(mailApiPort),
  };

  mkdirSync(runDir, { recursive: true });
  mkdirSync(resultsDir, { recursive: true });
  log(
    "journeys",
    `run ${runId}: project=${project} pg=${pgPort} rustfs=${rustfsPort} ` +
      `smtp=${smtpPort} mailpit-api=${mailApiPort} backend=${backendPort} web=${webPort}`,
  );
  log("journeys", `${selected.length} journey file(s): ${selected.join(", ")}`);

  // Builds first, before any container exists: they are minutes when cold,
  // and there is no reason to hold a database in RAM while cargo runs.
  //
  // Dev engine, as `e2e-parallel.mjs` builds for everything it does not
  // measure. A journey asks "does this behave", never "how fast". Choosing the
  // same profile also matters for coexisting with a sharded run: both write
  // `dist/engine`, and two runs asking for different profiles would rebuild it
  // under each other.
  await ensurePdfBuild({});
  await ensureEngineBuild({
    profile: engineProfile("dev"),
    noOpt: skipWasmOpt(),
  });
  await runCommand("cargo build -p thunderforge", {
    name: "build server",
    prefix: "journeys",
  });

  // `composeStarted` before `up`, not after: a half-started project is still a
  // project to tear down.
  composeStarted = true;
  // `--wait` blocks on Postgres's healthcheck and on the other two reaching
  // "running"; the explicit checks after it are for what `--wait` cannot see.
  execFileSync("docker", composeArgs("up", "-d", "--wait"), {
    stdio: "inherit",
    env: composeEnv,
  });
  assertContainerRunning("rustfs");
  assertContainerRunning("mailpit");
  // Any HTTP status counts for RustFS, as in torture: an anonymous request to
  // `/` is refused, and the refusal is proof enough that it is serving.
  await waitForUrl(`http://127.0.0.1:${rustfsPort}/`, "rustfs", 60_000, null, {
    anyStatus: true,
  });
  const mailpitApi = `http://127.0.0.1:${mailApiPort}`;
  await waitForUrl(`${mailpitApi}/api/v1/info`, "mailpit", 60_000);

  // Global setup applies `e2e_demo.sql` with `docker exec <container>`. A
  // compose project has no fixed container name — that is the point of it —
  // so the id is looked up and handed over rather than guessed.
  const postgresContainer = execFileSync(
    "docker",
    composeArgs("ps", "-q", "postgres"),
    { env: composeEnv, encoding: "utf-8" },
  ).trim();
  if (!postgresContainer)
    throw new Error("no postgres container in the project");

  const databaseUrl = `postgres://postgres:password@127.0.0.1:${pgPort}/thunderforge`;
  // A full migrate, since the database is empty every run — so a broken
  // migration fails here, named, rather than inside a journey.
  await runCommand("diesel migration run", {
    name: "migrate",
    cwd: join(ROOT_DIR, "src/server"),
    prefix: "journeys",
    env: { DATABASE_URL: databaseUrl },
  });

  // The same seeds an `e2e-parallel` template gets, in the same order:
  // `demo_accounts.sql` here (setup complete, a platform administrator), and
  // `e2e_demo.sql` from global setup below, which also clears `e2eadmin`'s
  // second factor so setup can enrol one the way a person does.
  execFileSync(
    "docker",
    [
      "exec",
      "-i",
      postgresContainer,
      "psql",
      "-U",
      "postgres",
      "-d",
      "thunderforge",
      "-v",
      "ON_ERROR_STOP=1",
      "-q",
    ],
    {
      input: readFileSync(
        join(ROOT_DIR, "src/server/seeds/demo_accounts.sql"),
        "utf-8",
      ),
      stdio: ["pipe", "ignore", "inherit"],
    },
  );

  const dataPath = join(runDir, "data");
  mkdirSync(dataPath, { recursive: true });
  linkPacks(dataPath);

  // What an `e2e-parallel` shard's backend is given (`startShard`'s `shared`),
  // pointed at this instance instead, less what only that suite needs (the
  // GitHub and OAuth stubs, the env-fixed realm name its settings spec pins).
  const backendEnv = {
    DATABASE_URL: databaseUrl,
    // Every RustFS variable, not just the endpoint. `dotenvy` searches parent
    // directories for a `.env`, and a worktree under a developer's checkout
    // inherits theirs — which names the dev RustFS. `dotenvy` never overwrites
    // a variable already present, so setting all of them here wins.
    RUSTFS_ENDPOINT: `http://127.0.0.1:${rustfsPort}`,
    RUSTFS_BUCKET: "tf-journeys",
    RUSTFS_REGION: "us-east-1",
    RUSTFS_ROOT_ACCESS_KEY:
      process.env.RUSTFS_ROOT_ACCESS_KEY ?? "thunderforge-rustfs-root",
    RUSTFS_ROOT_SECRET_KEY:
      process.env.RUSTFS_ROOT_SECRET_KEY ?? "thunderforge-rustfs-root-secret",
    THUNDERFORGE_DATA_PATH: dataPath,
    // Debug builds only (`auth_middleware::rate_limit_disabled`). A journey
    // registers a Game Master and a player and signs an operator in, all from
    // one IP, and a 429 there reads as a broken login page.
    THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT: "1",
    // One browser at a time drives this backend; see `startShard` for why the
    // default pool of 32 is wrong for a harness.
    DATABASE_POOL_MAX_SIZE: "8",
    // Mail goes to this run's Mailpit, fixed in the environment. A journey is
    // a person using a configured instance, not an operator configuring one,
    // so there is no reason to make each journey set it up first.
    THUNDERFORGE_SMTP_ENABLED: "true",
    THUNDERFORGE_SMTP_HOST: "127.0.0.1",
    THUNDERFORGE_SMTP_PORT: String(smtpPort),
    THUNDERFORGE_SMTP_SECURITY: "none",
    THUNDERFORGE_SMTP_FROM_ADDRESS: "journeys@example.test",
    // Cleared, as `startShard` clears them: a developer's `.env` with real
    // GitHub application credentials or a Keycloak would otherwise leak into
    // the instance, and a journey would pass or fail by whose laptop it ran on.
    // Blank rather than removed, because the registry and `oauth_env.rs` read
    // blank as unset and `dotenvy` would refill a removed one.
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
    FEEDBACK_GITHUB_APP_CLIENT_ID: "",
    FEEDBACK_GITHUB_APP_SLUG: "",
    FEEDBACK_GITHUB_APP_PRIVATE_KEY: "",
    FEEDBACK_GITHUB_APP_PRIVATE_KEY_FILE: "",
    FEEDBACK_GITHUB_APP_PRIVATE_KEY_BASE64: "",
    OAUTH_KEYCLOAK_ISSUER_URL: "",
    OAUTH_KEYCLOAK_CLIENT_ID: "",
    OAUTH_KEYCLOAK_CLIENT_SECRET: "",
    OAUTH_KEYCLOAK_LABEL: "",
  };

  const backend = startService(
    "backend",
    join(ROOT_DIR, "target/debug/thunderforge"),
    ["--port", String(backendPort)],
    { cwd: ROOT_DIR, env: backendEnv },
  );
  // `/api/readyz`: readiness says the database is reachable, which for a
  // database that did not exist a minute ago is the fact in question.
  await waitForUrl(
    `http://127.0.0.1:${backendPort}/api/readyz`,
    `the backend (log: ${relative(ROOT_DIR, backend.logPath)})`,
    180_000,
    backend.child,
  );
  log("journeys", "backend is up");

  const vite = startService(
    "vite",
    "pnpm",
    ["-F", "@thunderforge/web", "run", "dev"],
    {
      cwd: ROOT_DIR,
      env: {
        THUNDERFORGE_WEB_PORT: String(webPort),
        THUNDERFORGE_BACKEND_ORIGIN: `http://127.0.0.1:${backendPort}`,
      },
    },
  );
  const baseUrl = `http://127.0.0.1:${webPort}`;
  await waitForUrl(
    `${baseUrl}/`,
    `vite (log: ${relative(ROOT_DIR, vite.logPath)})`,
    180_000,
    vite.child,
  );
  log("journeys", `vite is up on ${baseUrl}`);

  // Global setup runs as part of this, from the journeys config: it applies
  // `e2e_demo.sql`, enrols `e2eadmin`'s second factor through the API and
  // writes the secret under this run's demo directory.
  const reportPath = join(resultsDir, "results.json");
  const started = Date.now();
  const code = await new Promise((resolve) => {
    const child = spawn(
      "pnpm",
      [
        "exec",
        "playwright",
        "test",
        "--config=playwright.journeys.config.ts",
        ...selected,
      ],
      {
        cwd: join(ROOT_DIR, "apps/web"),
        stdio: "inherit",
        // Its own process group, so Ctrl-C reaches this script (which tears
        // down) rather than racing it to kill the browsers mid-teardown.
        detached: true,
        env: {
          ...process.env,
          THUNDERFORGE_JOURNEYS_STACK: "1",
          THUNDERFORGE_JOURNEYS_RESULTS: resultsDir,
          PLAYWRIGHT_BASE_URL: baseUrl,
          PLAYWRIGHT_JSON_OUTPUT_NAME: reportPath,
          THUNDERFORGE_E2E_DEMO_DIR: join(runDir, "demo"),
          THUNDERFORGE_POSTGRES_CONTAINER: postgresContainer,
          THUNDERFORGE_DB_NAME: "thunderforge",
          THUNDERFORGE_DB_USER: "postgres",
          THUNDERFORGE_E2E_MAILPIT_API: mailpitApi,
          THUNDERFORGE_E2E_MAILPIT_SMTP_PORT: String(smtpPort),
        },
      },
    );
    services.push({ name: "playwright", child });
    child.once("exit", (exitCode, signal) =>
      resolve(exitCode ?? (signal ? 1 : 0)),
    );
  });

  const minutes = ((Date.now() - started) / 60_000).toFixed(1);
  const verdict = judge(code, reportPath);
  const counts = verdict.stats
    ? `${verdict.stats.expected} passed, ${verdict.stats.unexpected} failed, ${verdict.stats.skipped} skipped`
    : "no report";
  if (verdict.reasons.length > 0) {
    log(
      "journeys",
      `run ${runId} FAILED in ${minutes} min (${verdict.reasons.join(", ")}; ${counts}). ` +
        `Traces and screenshots: ${relative(ROOT_DIR, resultsDir)}`,
      process.stderr,
    );
    return 1;
  }
  log("journeys", `run ${runId} passed in ${minutes} min (${counts})`);
  return 0;
}

let exitCode = 1;
try {
  exitCode = await main();
} catch (error) {
  log(
    "journeys",
    `run ${runId} FAILED: ${error?.stack ?? error}`,
    process.stderr,
  );
  exitCode = 1;
} finally {
  await teardown();
}
process.exit(exitCode);
