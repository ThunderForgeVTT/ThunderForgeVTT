/**
 * A checkout's own e2e stack: Postgres and RustFS from `compose.e2e.yml`
 * (spec 089).
 *
 * # Why a stack per checkout
 *
 * Every worktree used to test against the one `thunderforge-postgres` from
 * `compose.dev.yml`. Two runs in two worktrees dropped and rebuilt each
 * other's templates and shard databases, which is why `f4263b49` made the
 * machine take one run at a time. A compose project per checkout gives each
 * its own server, volume and buckets, so their data can't meet.
 *
 * # Why the ports are read, never fixed
 *
 * Both services publish `127.0.0.1::<port>`: Docker picks a free host port.
 * It can pick another after a restart, so `ensureStack` reads them on every
 * run and nothing caches them between runs.
 *
 * # No fallback
 *
 * A stack that won't come up is an error naming the project and the service.
 * Falling back to the shared dev Postgres would bring the collisions back
 * without anyone noticing.
 */

import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { realpathSync } from "node:fs";
import { basename, join } from "node:path";

export const CHECKOUT_LABEL = "thunderforge.checkout";
const SERVICES = ["postgres", "rustfs"];
const DB_USER = "postgres";
const DB_PASSWORD = "password";

/**
 * `tf-<basename>-<hash>`: the basename lower-cased with everything outside
 * `[a-z0-9-]` turned into `-`, and the first 6 hex of the SHA-1 of the real
 * path, so two checkouts both called `ThunderForgeVTT` get two stacks.
 */
export function projectName(root) {
  const real = realpathSync(root);
  const name = basename(real)
    .toLowerCase()
    .replace(/[^a-z0-9-]/g, "-");
  const hash = createHash("sha1").update(real).digest("hex").slice(0, 6);
  return `tf-${name}-${hash}`;
}

/**
 * The host port in `docker compose port` output. Docker prints one line per
 * binding (`127.0.0.1:49153`, `[::1]:49153`); a binding off loopback is
 * refused, because this stack must never be reachable from another machine.
 */
export function parsePort(output) {
  const lines = String(output)
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  if (lines.length === 0) {
    throw new Error("docker compose port printed no published port");
  }
  const loopback = lines.find(
    (line) => line.startsWith("127.0.0.1:") || line.startsWith("[::1]:"),
  );
  if (!loopback) {
    throw new Error(
      `the port is published off loopback (${lines.join(", ")}); ` +
        "compose.e2e.yml must bind 127.0.0.1",
    );
  }
  const text = loopback.slice(loopback.lastIndexOf(":") + 1);
  const port = Number(text);
  if (!/^\d+$/.test(text) || port < 1 || port > 65535) {
    throw new Error(`"${loopback}" ends in something that is not a port`);
  }
  return port;
}

/** The compose files for this run: the stack, plus the tmpfs override if asked. */
export function composeFiles(root, env = process.env) {
  const files = [join(root, "compose.e2e.yml")];
  if (env.THUNDERFORGE_E2E_TMPFS === "1") {
    files.push(join(root, "compose.e2e.tmpfs.yml"));
  }
  return files;
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: "utf-8", ...options });
  if (result.error) {
    return { status: 127, stdout: "", stderr: String(result.error.message) };
  }
  return {
    status: result.status ?? 1,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
  };
}

/**
 * Brings the checkout's stack up, waits for both services to be healthy and
 * reads their ports.
 *
 * Returns `{ project, postgresPort, databaseUrl(name), psql(db, sql),
 * rustfsEndpoint }`. Throws, naming the project and the failing service with
 * its last 20 log lines, if the stack won't come up.
 */
export function ensureStack(root, env = process.env) {
  const checkout = realpathSync(root);
  const project = projectName(checkout);
  const composeEnv = { ...env, THUNDERFORGE_CHECKOUT: checkout };
  const base = ["compose", "-p", project];
  for (const file of composeFiles(checkout, env)) base.push("-f", file);
  const compose = (args) =>
    run("docker", [...base, ...args], { env: composeEnv, cwd: checkout });

  const up = compose(["up", "-d", "--wait", "--wait-timeout", "120"]);
  if (up.status !== 0) {
    throw new Error(stackFailure(project, compose, up));
  }

  const port = (service, target) => {
    const out = compose(["port", service, String(target)]);
    if (out.status !== 0) {
      throw new Error(
        `e2e stack ${project}: no port for ${service}:${target}: ${out.stderr.trim()}`,
      );
    }
    return parsePort(out.stdout);
  };
  const postgresPort = port("postgres", 5432);
  const rustfsPort = port("rustfs", 9000);

  // `docker exec` by container name: `docker compose exec` would need the
  // compose files on every call, and psql runs hundreds of times a run. The
  // specs' own `docker exec`s get the same name through the shard env.
  const ids = compose(["ps", "--format", "{{.Name}}", "postgres"]);
  const postgresContainer = ids.stdout.trim().split("\n")[0];
  if (ids.status !== 0 || !postgresContainer) {
    throw new Error(`e2e stack ${project}: postgres has no running container`);
  }

  return {
    project,
    checkout,
    postgresPort,
    postgresContainer,
    rustfsEndpoint: `http://127.0.0.1:${rustfsPort}`,
    databaseUrl: (name) =>
      `postgres://${DB_USER}:${DB_PASSWORD}@127.0.0.1:${postgresPort}/${name}`,
    psql(database, sql) {
      const out = run(
        "docker",
        [
          "exec",
          "-i",
          postgresContainer,
          "psql",
          "-U",
          DB_USER,
          "-d",
          database,
          "-v",
          "ON_ERROR_STOP=1",
          "-q",
        ],
        { input: sql },
      );
      if (out.status !== 0) {
        throw new Error(
          `psql in ${project} (${database}) failed: ${out.stderr.trim()}`,
        );
      }
      return out.stdout;
    },
  };
}

/** Which service is down, with its log tail and the command that retries. */
function stackFailure(project, compose, up) {
  const unhealthy = [];
  const ps = compose(["ps", "-a", "--format", "json"]);
  for (const line of ps.stdout.split("\n").filter(Boolean)) {
    try {
      const row = JSON.parse(line);
      if (row.State !== "running" || (row.Health && row.Health !== "healthy")) {
        unhealthy.push(row.Service);
      }
    } catch {
      // Not a row; older compose printed a JSON array. Fall through to both.
    }
  }
  const services = unhealthy.length > 0 ? unhealthy : SERVICES;
  const sections = services.map((service) => {
    const logs = compose(["logs", "--no-color", "--tail", "20", service]);
    return `--- ${service} (last 20 lines) ---\n${(logs.stdout + logs.stderr).trim()}`;
  });
  return [
    `e2e stack ${project} did not come up healthy: ${services.join(", ")}.`,
    up.stderr.trim(),
    ...sections,
    `Retry with: docker compose -p ${project} -f compose.e2e.yml up -d --wait`,
  ]
    .filter(Boolean)
    .join("\n");
}
