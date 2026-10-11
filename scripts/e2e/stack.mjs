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
import { existsSync, realpathSync } from "node:fs";
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
        "the compose file must bind 127.0.0.1",
    );
  }
  const text = loopback.slice(loopback.lastIndexOf(":") + 1);
  const port = Number(text);
  if (!/^\d+$/.test(text) || port < 1 || port > 65535) {
    throw new Error(`"${loopback}" ends in something that is not a port`);
  }
  return port;
}

/**
 * The host port Docker gave `service`'s `containerPort`, for a compose project
 * whose files publish `127.0.0.1::<port>`. `composeArgs` is everything up to
 * the subcommand (`["-f", file, "-p", project]`).
 */
export function publishedPort(composeArgs, service, containerPort, env) {
  const out = spawnSync(
    "docker",
    ["compose", ...composeArgs, "port", service, String(containerPort)],
    { encoding: "utf-8", env },
  );
  if (out.status !== 0) {
    throw new Error(
      `docker compose port ${service} ${containerPort}: ${out.stderr.trim()}`,
    );
  }
  return parsePort(out.stdout);
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
  const container = (service) => {
    const ids = compose(["ps", "--format", "{{.Name}}", service]);
    const name = ids.stdout.trim().split("\n")[0];
    if (ids.status !== 0 || !name) {
      throw new Error(`e2e stack ${project}: ${service} has no running container`);
    }
    return name;
  };
  const postgresContainer = container("postgres");
  const rustfsContainer = container("rustfs");

  return {
    project,
    checkout,
    postgresPort,
    postgresContainer,
    rustfsEndpoint: `http://127.0.0.1:${rustfsPort}`,
    /**
     * Make `bucket` if it is missing. Only the app binary does that otherwise,
     * and `cargo test`'s storage tests expect `thunderforge-canvas-assets` to
     * exist. Signed with curl's own SigV4, from inside the container, so the
     * host needs no S3 tool.
     */
    ensureBucket(bucket) {
      const access = env.RUSTFS_ROOT_ACCESS_KEY || "thunderforge-rustfs-root";
      const secret =
        env.RUSTFS_ROOT_SECRET_KEY || "thunderforge-rustfs-root-secret";
      const out = run("docker", [
        "exec",
        rustfsContainer,
        "curl",
        "-s",
        "-o",
        "/dev/null",
        "-w",
        "%{http_code}",
        "-X",
        "PUT",
        "--aws-sigv4",
        "aws:amz:us-east-1:s3",
        "--user",
        `${access}:${secret}`,
        `http://127.0.0.1:9000/${bucket}`,
      ]);
      // 409: BucketAlreadyOwnedByYou.
      if (!["200", "409"].includes(out.stdout.trim())) {
        throw new Error(
          `e2e stack ${project}: could not make bucket ${bucket} (HTTP ${out.stdout.trim() || "?"}) ${out.stderr.trim()}`,
        );
      }
    },
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

const UNITS = { B: 1, kB: 1e3, KB: 1e3, MB: 1e6, GB: 1e9, TB: 1e12 };

/** Bytes in one of docker's human sizes (`180MB`, `2.1kB`); 0 for anything else. */
export function parseSize(text) {
  const match = /^([\d.]+)\s*([kKMGT]?B)$/.exec(String(text).trim());
  return match ? Number(match[1]) * UNITS[match[2]] : 0;
}

/**
 * Every `tf-*` stack from what docker reports (T040, FR-012):
 *
 * - `projects`: `docker compose ls --all --format json`;
 * - `labels`: `project<TAB>checkout` lines, one per labelled container;
 * - `volumes`: the `Volumes` of `docker system df -v --format json`.
 *
 * A project with only volumes left (its containers removed) is listed too,
 * since its volumes are what take the space. Its checkout is unknown — the
 * label lives on containers — so `gone` is null, not true.
 */
export function summarizeStacks({ projects, labels, volumes }, exists = existsSync) {
  const stacks = new Map();
  const stackFor = (project) => {
    if (!stacks.has(project)) {
      stacks.set(project, {
        project,
        status: "volumes only",
        checkout: null,
        gone: null,
        volumes: [],
        bytes: 0,
      });
    }
    return stacks.get(project);
  };
  for (const { Name, Status } of projects) {
    if (Name.startsWith("tf-")) stackFor(Name).status = Status;
  }
  for (const line of String(labels).split("\n")) {
    const [project, checkout] = line.split("\t");
    if (project?.startsWith("tf-") && checkout) stackFor(project).checkout = checkout;
  }
  for (const volume of volumes) {
    const project = /(?:^|,)com\.docker\.compose\.project=([^,]+)/.exec(
      volume.Labels ?? "",
    )?.[1];
    if (!project?.startsWith("tf-")) continue;
    const stack = stackFor(project);
    stack.volumes.push({ name: volume.Name, size: volume.Size });
    stack.bytes += parseSize(volume.Size);
  }
  for (const stack of stacks.values()) {
    if (stack.checkout) stack.gone = !exists(stack.checkout);
  }
  return [...stacks.values()].sort((a, b) => a.project.localeCompare(b.project));
}

/** Ask docker. `sizes: false` skips `system df -v`, which can take a minute. */
export function listStacks({ sizes = true } = {}) {
  const ls = run("docker", ["compose", "ls", "--all", "--format", "json"]);
  if (ls.status !== 0) throw new Error(`docker compose ls failed: ${ls.stderr.trim()}`);
  const labels = run("docker", [
    "ps",
    "-a",
    "--filter",
    `label=${CHECKOUT_LABEL}`,
    "--format",
    `{{.Label "com.docker.compose.project"}}\t{{.Label "${CHECKOUT_LABEL}"}}`,
  ]);
  let volumes = [];
  if (sizes) {
    const df = run("docker", ["system", "df", "-v", "--format", "json"]);
    if (df.status === 0) volumes = JSON.parse(df.stdout).Volumes ?? [];
  } else {
    const vols = run("docker", [
      "volume",
      "ls",
      "--filter",
      "label=com.docker.compose.project",
      "--format",
      "{{.Name}}\t{{.Labels}}",
    ]);
    volumes = vols.stdout
      .split("\n")
      .filter(Boolean)
      .map((line) => {
        const [Name, Labels] = line.split("\t");
        return { Name, Labels, Size: "N/A" };
      });
  }
  return summarizeStacks({
    projects: JSON.parse(ls.stdout || "[]"),
    labels: labels.stdout,
    volumes,
  });
}

function humanBytes(bytes) {
  for (const [unit, size] of [["GB", 1e9], ["MB", 1e6], ["kB", 1e3]]) {
    if (bytes >= size) return `${(bytes / size).toFixed(1)}${unit}`;
  }
  return `${bytes}B`;
}

function describeStack(stack) {
  const where =
    stack.checkout === null
      ? "checkout unknown (no container left)"
      : `${stack.checkout}${stack.gone ? "  [GONE]" : ""}`;
  return `${stack.project}  ${stack.status}  ${humanBytes(stack.bytes)}  ${where}`;
}

/**
 * `TEST_DATABASE_URL` and `RUSTFS_ENDPOINT` for `cargo test` against this
 * checkout's stack (T043, plan.md Decision 6). The database is the same
 * `thunderforge_test` the default run uses, on this checkout's server.
 */
export function stackEnv(root, env = process.env) {
  const stack = ensureStack(root, env);
  stack.ensureBucket("thunderforge-canvas-assets");
  return {
    TEST_DATABASE_URL: stack.databaseUrl("thunderforge_test"),
    RUSTFS_ENDPOINT: stack.rustfsEndpoint,
  };
}

async function confirm(question) {
  if (!process.stdin.isTTY) return false;
  const { createInterface } = await import("node:readline/promises");
  const rl = createInterface({ input: process.stdin, output: process.stdout });
  try {
    return /^y(es)?$/i.test((await rl.question(question)).trim());
  } finally {
    rl.close();
  }
}

/** Remove one stack, after showing what goes and hearing `y` (FR-012). */
async function removeStack(project) {
  if (!/^tf-[a-z0-9-]+$/.test(project ?? "")) {
    throw new Error(`"${project}" is not an e2e stack name (tf-…); see make e2e-stacks`);
  }
  const stack = listStacks().find((s) => s.project === project);
  if (!stack) throw new Error(`no e2e stack named ${project}; see make e2e-stacks`);
  const containers = run("docker", [
    "ps",
    "-a",
    "--filter",
    `label=com.docker.compose.project=${project}`,
    "--format",
    "{{.Names}}",
  ]).stdout.trim();
  const mailpits = run("docker", [
    "ps",
    "-a",
    "--filter",
    `name=${project}-mailpit-`,
    "--format",
    "{{.Names}}",
  ]).stdout.trim();
  console.log(`This removes e2e stack ${describeStack(stack)}:`);
  for (const name of [...containers.split("\n"), ...mailpits.split("\n")].filter(Boolean)) {
    console.log(`  container ${name}`);
  }
  console.log(`  network   ${project}_default`);
  for (const volume of stack.volumes) console.log(`  volume    ${volume.name} (${volume.size})`);
  if (!(await confirm("Remove it, data included? [y/N] "))) {
    console.log("Left as it is.");
    return;
  }
  for (const name of mailpits.split("\n").filter(Boolean)) run("docker", ["rm", "-f", name]);
  const down = run("docker", ["compose", "-p", project, "down", "-v", "--remove-orphans"]);
  if (down.status !== 0) throw new Error(`docker compose down failed: ${down.stderr.trim()}`);
  console.log(`Removed ${project}.`);
}

/**
 * The command line, for the Makefile:
 *
 *     stack.mjs list        every tf-* stack (make e2e-stacks)
 *     stack.mjs rm <name>   remove one, after a y/N (make e2e-stacks-rm)
 *     stack.mjs gone-note   one line when stacks belong to deleted checkouts (make gc)
 *     stack.mjs env         TEST_DATABASE_URL and RUSTFS_ENDPOINT, shell-quoted
 */
if (process.argv[1] && realpathSync(process.argv[1]) === realpathSync(new URL(import.meta.url).pathname)) {
  const [command, arg] = process.argv.slice(2);
  try {
    if (command === "list") {
      const stacks = listStacks();
      if (stacks.length === 0) console.log("No e2e stacks.");
      for (const stack of stacks) console.log(describeStack(stack));
    } else if (command === "rm") {
      await removeStack(arg);
    } else if (command === "gone-note") {
      const gone = listStacks({ sizes: false }).filter((s) => s.gone);
      if (gone.length > 0) {
        console.log(
          `gc: ${gone.length} e2e stack${gone.length === 1 ? " belongs" : "s belong"} ` +
            "to deleted checkouts: make e2e-stacks",
        );
      }
    } else if (command === "env") {
      const root = join(new URL(import.meta.url).pathname, "..", "..", "..");
      for (const [key, value] of Object.entries(stackEnv(root))) {
        console.log(`${key}='${value}'`);
      }
    } else {
      console.error("usage: stack.mjs list | rm <tf-project> | gone-note | env");
      process.exit(2);
    }
  } catch (error) {
    console.error(String(error.message ?? error));
    process.exit(1);
  }
}
