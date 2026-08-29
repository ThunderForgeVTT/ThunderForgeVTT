#!/usr/bin/env node
/**
 * Run a load/torture tier against a throwaway stack.
 *
 *   node scripts/torture.mjs 5      # cheap, run it often
 *   node scripts/torture.mjs 100    # the ceiling check
 *
 * Everything it creates is ephemeral. Postgres and RustFS come up on tmpfs in
 * an isolated compose project on non-default ports, so a run cannot touch —
 * or be confused with — your dev database, and teardown is just stopping the
 * containers. Nothing to garbage-collect afterwards, which matters because
 * the alternative is a dozen stale volumes after a week of tuning.
 *
 * The project name is unique per run, so two runs can overlap without
 * colliding (they will compete for CPU, which is its own problem, but they
 * will not corrupt each other).
 */
import { spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import process from "node:process";

const TIERS = [5, 10, 25, 50, 100, 250, 500, 1000];

const requested = Number(process.argv[2] ?? "5");
if (!TIERS.includes(requested)) {
  console.error(
    `Unknown tier "${process.argv[2]}". Choose one of: ${TIERS.join(", ")}`,
  );
  process.exit(2);
}

const runId = randomBytes(4).toString("hex");
const project = `tf-torture-${runId}`;
// Derived from the run id so overlapping runs do not fight over ports.
const pgPort = 55_000 + (parseInt(runId, 16) % 400);
const rustfsPort = 59_000 + (parseInt(runId, 16) % 400);

const composeEnv = {
  ...process.env,
  TORTURE_PG_PORT: String(pgPort),
  TORTURE_RUSTFS_PORT: String(rustfsPort),
};

function run(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      stdio: "inherit",
      ...options,
    });
    child.on("error", reject);
    child.on("exit", (code) =>
      code === 0
        ? resolve()
        : reject(new Error(`${command} ${args.join(" ")} exited ${code}`)),
    );
  });
}

const compose = (...args) =>
  run(
    "docker",
    ["compose", "-f", "compose.torture.yml", "-p", project, ...args],
    {
      env: composeEnv,
    },
  );

async function waitForPostgres() {
  // Poll the container's own healthcheck rather than sleeping. A fixed sleep
  // is either too short on a cold image pull or wasted time on a warm one.
  for (let attempt = 0; attempt < 60; attempt += 1) {
    try {
      await run(
        "docker",
        [
          "compose",
          "-f",
          "compose.torture.yml",
          "-p",
          project,
          "exec",
          "-T",
          "postgres",
          "pg_isready",
          "-U",
          "postgres",
          "-d",
          "thunderforge",
        ],
        { env: composeEnv, stdio: "ignore" },
      );
      return;
    } catch {
      await new Promise((r) => setTimeout(r, 1_000));
    }
  }
  // Dump the container's own logs before giving up. Without this the failure
  // is just "never became ready", which says nothing about why — and the
  // containers are torn down immediately afterwards, taking the evidence
  // with them.
  console.error("[torture] postgres never became ready; its logs follow:");
  await run(
    "docker",
    ["compose", "-f", "compose.torture.yml", "-p", project, "logs", "postgres"],
    { env: composeEnv },
  ).catch(() => {});
  throw new Error("postgres never became ready");
}

/**
 * Wait for a service to answer on a port.
 *
 * Any HTTP status counts, 404 included: the question is whether something is
 * listening and serving, not whether the path exists.
 */
async function waitForHttp(url, what, attempts = 120) {
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    try {
      await fetch(url);
      return;
    } catch {
      await new Promise((r) => setTimeout(r, 1_000));
    }
  }
  throw new Error(`${what} never answered on ${url}`);
}

/**
 * Fail loudly if a container died on startup.
 *
 * RustFS spent this harness's entire history exiting immediately with a
 * permission error, and nothing noticed — because nothing was reaching it
 * (see below) and `docker compose up -d` returns success for a container
 * that starts and then dies. A load run whose object storage is absent is
 * not a load run.
 */
async function assertContainerRunning(service) {
  const state = await new Promise((resolve, reject) => {
    const child = spawn(
      "docker",
      [
        "compose",
        "-f",
        "compose.torture.yml",
        "-p",
        project,
        "ps",
        "-a",
        "--format",
        "{{.State}}",
        service,
      ],
      { env: composeEnv, stdio: ["ignore", "pipe", "inherit"] },
    );
    let out = "";
    child.stdout.on("data", (chunk) => (out += chunk));
    child.on("error", reject);
    child.on("exit", () => resolve(out.trim()));
  });
  if (!state.startsWith("running")) {
    console.error(`[torture] ${service} is "${state}"; its logs follow:`);
    await run(
      "docker",
      ["compose", "-f", "compose.torture.yml", "-p", project, "logs", service],
      { env: composeEnv },
    ).catch(() => {});
    throw new Error(`${service} is not running`);
  }
}

const databaseUrl = `postgres://postgres:password@localhost:${pgPort}/thunderforge`;

let started = false;
let backendProcess = null;
try {
  console.log(
    `[torture] tier=${requested} project=${project} pg=${pgPort} rustfs=${rustfsPort}`,
  );

  await compose("up", "-d");
  started = true;
  await waitForPostgres();
  // `up -d` succeeds for a container that starts and immediately dies, which
  // is exactly what RustFS was doing here.
  await assertContainerRunning("rustfs");
  await waitForHttp(`http://localhost:${rustfsPort}/`, "rustfs");

  // Schema. The stack is empty every run, so this is a full migrate, not an
  // incremental one — which incidentally means a broken migration fails here
  // rather than halfway through a load test.
  await run("diesel", ["migration", "run"], {
    env: { ...composeEnv, DATABASE_URL: databaseUrl },
  });

  // Make the instance look already-installed.
  //
  // A migrated database is not a usable one: with no admin, the server's
  // `setup_status` answers `setup_required: true` and the app redirects every
  // route — `/register` included — to `/setup`. The storm's first act is to
  // register five users, so it sat for three minutes on a form that was not
  // on the page.
  //
  // It has to be an **admin user**, not a completed bootstrap row. Seeding
  // the row does not survive startup: `ensure_admin_bootstrap_code` runs on
  // every boot and, finding no admin, sets `setup_completed_at` back to NULL
  // and mints a fresh bootstrap code (`auth/mod.rs`). `admin_exists` is the
  // only condition that short-circuits it.
  //
  // The password hash is deliberately not a real one. Nothing ever
  // authenticates as this account — the storm registers its own users — and
  // a hash that cannot verify is the honest way to say so.
  await run(
    "docker",
    [
      "compose",
      "-f",
      "compose.torture.yml",
      "-p",
      project,
      "exec",
      "-T",
      "postgres",
      "psql",
      "-U",
      "postgres",
      "-d",
      "thunderforge",
      "-v",
      "ON_ERROR_STOP=1",
      "-c",
      "INSERT INTO users (id, username, password_hash, email, is_admin, " +
        "created_at, updated_at) VALUES (gen_random_uuid(), 'torture-admin', " +
        "'x-not-a-usable-hash', 'torture-admin@example.invalid', true, now(), now()) " +
        "ON CONFLICT DO NOTHING;",
    ],
    { env: composeEnv },
  );

  const stackEnv = {
    ...composeEnv,
    DATABASE_URL: databaseUrl,
    RUSTFS_ENDPOINT: `http://localhost:${rustfsPort}`,
    TORTURE_SESSIONS: String(requested),
    // Registering a table of players legitimately exceeds a limit written
    // for humans typing passwords — 15 auth requests per minute per IP. The
    // tests still pace themselves against it (see `table-storm`), so this is
    // belt and braces rather than a licence to hammer: it keeps a large tier
    // from spending most of its runtime asleep.
    //
    // Safe here for two reasons that both have to hold: the backend below is
    // a debug build, and a release build does not contain this code path at
    // all (`auth_middleware::rate_limit_disabled` is `cfg(debug_assertions)`).
    // The stack is also on a throwaway database on a random port that dies
    // with the run.
    THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT: "1",
  };

  // The backend, started here rather than by Playwright.
  //
  // This is the whole reason the isolation above was fiction. Playwright's
  // `webServer` runs `pnpm run dev` with `apps/web` as its directory, and
  // *that* `dev` script is one word: `vite`. So no process ever read the
  // `DATABASE_URL` this script works so hard to create. With a dev stack
  // running, every storm hit the dev backend and the dev database through
  // vite's `/api` proxy, and `reuseExistingServer: false` did nothing about
  // it, because it only ever governed vite. With no dev stack running, the
  // run simply failed with ECONNREFUSED.
  //
  // Started before Playwright and waited for, which also removes the trap
  // that a run beginning before the backend is up dies in `globalSetup`.
  console.log("[torture] starting the backend against the throwaway stack");
  const backend = spawn("cargo", ["run", "--bin", "thunderforge"], {
    stdio: "inherit",
    env: stackEnv,
  });
  backendProcess = backend;
  const backendExited = new Promise((_, reject) => {
    backend.on("exit", (code) =>
      reject(new Error(`backend exited ${code} before the run started`)),
    );
    backend.on("error", reject);
  });
  await Promise.race([
    waitForHttp("http://localhost:30000/", "the backend", 300),
    backendExited,
  ]);
  console.log("[torture] backend is up");

  // `TORTURE_SPECS` narrows the run to named specs.
  //
  // Above tier ~100 this stops being optional. `table-storm` and
  // `authority-storm` open one *browser context* per participant, so a tier of
  // 1000 asks for a thousand Chromium instances — which measures the machine,
  // not the server. The other three open their sockets inside one page and
  // scale to whatever the transport will bear, which is the thing worth
  // knowing at that size.
  const only = (process.env.TORTURE_SPECS ?? "").trim();
  const specArgs = only
    ? only.split(/\s+/).map((name) => `e2e/torture/${name}.torture.spec.ts`)
    : [];
  if (specArgs.length > 0) {
    console.log(`[torture] restricted to: ${only}`);
  }

  await run(
    "npx",
    [
      "playwright",
      "test",
      "--config=playwright.torture.config.ts",
      ...specArgs,
    ],
    {
      cwd: "apps/web",
      env: stackEnv,
    },
  );

  console.log(`[torture] tier=${requested} passed`);
} catch (error) {
  console.error(`[torture] tier=${requested} FAILED: ${error.message}`);
  process.exitCode = 1;
} finally {
  if (backendProcess && backendProcess.exitCode === null) {
    console.log("[torture] stopping the backend");
    backendProcess.kill("SIGTERM");
  }
  if (started) {
    // `-v` as well as `down`: tmpfs leaves nothing behind, but if someone
    // later swaps tmpfs for a volume this keeps the promise on the tin.
    console.log("[torture] tearing down");
    await compose("down", "-v", "--remove-orphans").catch(() => {
      console.error(
        `[torture] teardown failed — clean up by hand:\n` +
          `  docker compose -f compose.torture.yml -p ${project} down -v`,
      );
    });
  }
}
