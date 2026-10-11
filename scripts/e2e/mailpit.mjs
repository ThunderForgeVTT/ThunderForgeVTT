/**
 * One shard's mail sink: a real SMTP server, in a container of its own.
 *
 * # Why a container per shard rather than the compose one
 *
 * `compose.yml` runs a single Mailpit for `make dev`. Pointing four shards at
 * it would mean every shard's assertions read every other shard's messages,
 * and `clearInbox` — which the fixtures contract requires between tests —
 * would delete a neighbour's evidence mid-assertion.
 *
 * # Names and ports (spec 089)
 *
 * The name is `<project>-mailpit-<index>`, `<project>` being the checkout's
 * e2e stack (`stack.mjs`), and the container carries the same checkout label
 * as the stack. Two checkouts' runs can't meet on a name, and the checkout
 * lock means two runs never share a project. Both ports are published as
 * `127.0.0.1::<port>`: Docker picks free ones and they are read back, so two
 * runs can't meet on a port either, and nothing is reachable off the machine.
 *
 * # Leftovers
 *
 * A run killed with `kill -9` leaves its containers behind. On 2026-09-14 a
 * `docker rm -f` returned while an earlier `--rm` container of the same name
 * was still being removed, and `docker run --name` hit a conflict. So a
 * leftover of this project's index is removed and *waited for* until docker
 * no longer knows it. One whose pid label names a live run is never removed;
 * the error says so. A conflict that survives all of that is retried once,
 * then reported with docker's own message.
 */

import { spawnSync } from "node:child_process";

import { CHECKOUT_LABEL, parsePort } from "./stack.mjs";

const LABEL_PID = "thunderforge.e2e.pid";
const CONFLICT =
  /Conflict\.|is already in use|already allocated|address already in use|removal of container .* is already in progress/i;

function docker(args) {
  const result = spawnSync("docker", args, { encoding: "utf-8" });
  if (result.error) {
    return { status: 127, stdout: "", stderr: String(result.error.message) };
  }
  return {
    status: result.status ?? 1,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
  };
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function pidAlive(pid) {
  if (!Number.isInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return error?.code === "EPERM";
  }
}

/** Shard `index`'s container name in compose project `project`. */
export function mailpitName(project, index) {
  return `${project}-mailpit-${index}`;
}

/** `[{ name, pid }]` for containers matching a `docker ps -a` filter. */
function listContainers(filter) {
  const { status, stdout } = docker([
    "ps",
    "-a",
    "--filter",
    filter,
    "--format",
    `{{.Names}}\t{{.Label "${LABEL_PID}"}}`,
  ]);
  if (status !== 0) return [];
  return stdout
    .split("\n")
    .filter(Boolean)
    .map((line) => {
      const [name, pid] = line.split("\t");
      return { name, pid: Number(pid) || null };
    });
}

/** Remove `name` and resolve once docker no longer has a container by it. */
export async function removeAndWait(name, timeoutMs = 30_000) {
  docker(["rm", "-f", name]);
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (docker(["container", "inspect", name]).status !== 0) return true;
    await sleep(250);
    // A removal already in progress (`--rm` on exit) can make the first
    // `rm -f` fail; asking again is harmless once it is done.
    docker(["rm", "-f", name]);
  }
  return false;
}

/**
 * Clear what an earlier run of this checkout left under `name`. Throws,
 * rather than removing, when the leftover belongs to a live run.
 */
async function clearLeftovers(name) {
  // `name=` filters by substring: `-mailpit-1` would match `-mailpit-10`.
  for (const container of listContainers(`name=${name}`)) {
    if (container.name !== name) continue;
    if (
      container.pid &&
      container.pid !== process.pid &&
      pidAlive(container.pid)
    ) {
      throw new Error(
        `container "${name}" belongs to an e2e run that is still alive (pid ${container.pid}). ` +
          "Two runs from one checkout cannot share a stack; wait for it to finish.",
      );
    }
    if (!(await removeAndWait(name))) {
      throw new Error(
        `leftover container "${name}" would not go away within 30s ` +
          "(`docker ps -a` shows it); remove it by hand and run again.",
      );
    }
  }
}

/** The host port Docker published for `name`'s container port `target`. */
function publishedPort(name, target) {
  const out = docker(["port", name, `${target}/tcp`]);
  if (out.status !== 0) {
    throw new Error(`mailpit "${name}": no port for ${target}: ${out.stderr.trim()}`);
  }
  return parsePort(out.stdout);
}

/**
 * Start shard `index`'s Mailpit in `project` and return `{ name, smtpPort,
 * apiPort }`.
 *
 * `onCreated` is called as soon as docker has created the container, before
 * any readiness check, because a container that started and never answered is
 * exactly the one that must still be torn down.
 */
export async function startMailpitContainer({
  project,
  checkout,
  index,
  image,
  onCreated,
  log = () => {},
}) {
  const name = mailpitName(project, index);
  const run = () =>
    docker([
      "run",
      "--rm",
      "-d",
      "--name",
      name,
      "--label",
      `${LABEL_PID}=${process.pid}`,
      "--label",
      `${CHECKOUT_LABEL}=${checkout}`,
      "-p",
      "127.0.0.1::1025",
      "-p",
      "127.0.0.1::8025",
      // The dev stack's settings: Mailpit's SMTP listener is plaintext, which
      // is what the `none` security option exists for, and it accepts any
      // credentials so a spec can prove the username/password path is wired
      // without a real account.
      "-e",
      "MP_SMTP_AUTH_ACCEPT_ANY=1",
      "-e",
      "MP_SMTP_AUTH_ALLOW_INSECURE=1",
      image,
    ]);

  for (let attempt = 1; ; attempt += 1) {
    await clearLeftovers(name);
    const result = run();
    if (result.status === 0) {
      onCreated?.(name);
      return {
        name,
        smtpPort: publishedPort(name, 1025),
        apiPort: publishedPort(name, 8025),
      };
    }
    const message = result.stderr.trim() || `exit ${result.status}`;
    if (attempt === 1 && CONFLICT.test(message)) {
      log(
        `mailpit ${index}: docker run conflicted (${message.split("\n")[0]}); clearing leftovers and retrying once.`,
      );
      await removeAndWait(name);
      continue;
    }
    throw new Error(
      `mailpit ${index} could not start: \`docker run\` failed${
        attempt > 1 ? " twice" : ""
      }: ${message}` +
        (CONFLICT.test(message)
          ? ` — something keeps holding the name "${name}"; \`docker ps -a --filter name=${name}\` shows what.`
          : ""),
    );
  }
}

/** Remove containers by name, without waiting. Safe from a signal handler. */
export function removeContainers(names) {
  for (const name of names) docker(["rm", "-f", name]);
}
