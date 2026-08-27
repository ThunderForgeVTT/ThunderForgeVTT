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

const TIERS = [5, 10, 25, 50, 100];

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
  run("docker", ["compose", "-f", "compose.torture.yml", "-p", project, ...args], {
    env: composeEnv,
  });

async function waitForPostgres() {
  // Poll the container's own healthcheck rather than sleeping. A fixed sleep
  // is either too short on a cold image pull or wasted time on a warm one.
  for (let attempt = 0; attempt < 60; attempt += 1) {
    try {
      await run(
        "docker",
        ["compose", "-f", "compose.torture.yml", "-p", project, "exec", "-T",
         "postgres", "pg_isready", "-U", "postgres", "-d", "thunderforge"],
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

const databaseUrl = `postgres://postgres:password@localhost:${pgPort}/thunderforge`;

let started = false;
try {
  console.log(`[torture] tier=${requested} project=${project} pg=${pgPort} rustfs=${rustfsPort}`);

  await compose("up", "-d");
  started = true;
  await waitForPostgres();

  // Schema. The stack is empty every run, so this is a full migrate, not an
  // incremental one — which incidentally means a broken migration fails here
  // rather than halfway through a load test.
  await run("diesel", ["migration", "run"], {
    env: { ...composeEnv, DATABASE_URL: databaseUrl },
  });

  await run(
    "npx",
    ["playwright", "test", "--config=playwright.torture.config.ts"],
    {
      cwd: "apps/web",
      env: {
        ...composeEnv,
        DATABASE_URL: databaseUrl,
        RUSTFS_ENDPOINT: `http://localhost:${rustfsPort}`,
        TORTURE_SESSIONS: String(requested),
      },
    },
  );

  console.log(`[torture] tier=${requested} passed`);
} catch (error) {
  console.error(`[torture] tier=${requested} FAILED: ${error.message}`);
  process.exitCode = 1;
} finally {
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
