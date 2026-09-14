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
 * # Why the name carries this run's pid
 *
 * It used to be `thunderforge-e2e-mailpit-<index>`, the same every run, so the
 * next run could `docker rm -f` a corpse by name. On 2026-09-14 that raced:
 * `docker rm -f` returned while an earlier `--rm` container of that name was
 * still being removed, `docker run --name` hit a conflict, and the uncaught
 * error took the whole harness down with no summary for the lane.
 *
 * So the name is unique per run and cannot collide at all. Leftovers from
 * earlier runs are still found — by the index prefix, and by who publishes the
 * ports — removed, and *waited for* until docker no longer knows them. A
 * leftover that belongs to a run that is still alive (its label names a live
 * pid) is never removed; that is a second run, and the error says so.
 *
 * A conflict that survives all of that is retried once, and then reported with
 * docker's own message and the command that shows the culprit.
 */

import { spawnSync } from "node:child_process";

export const MAILPIT_PREFIX = "thunderforge-e2e-mailpit-";
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
 * Clear what earlier runs left on this index or these ports.
 *
 * Throws, rather than removing, when a leftover belongs to a live run or is
 * not one of this harness's containers at all.
 */
async function clearLeftovers(index, ports) {
  const mine = new RegExp(`^${MAILPIT_PREFIX}${index}(-.+)?$`);
  const byName = listContainers(`name=${MAILPIT_PREFIX}`).filter((c) =>
    mine.test(c.name),
  );
  const byPort = ports.flatMap((port) =>
    listContainers(`publish=${port}`).map((c) => ({ ...c, port })),
  );

  const foreign = byPort.filter((c) => !c.name.startsWith(MAILPIT_PREFIX));
  if (foreign.length) {
    throw new Error(
      foreign
        .map(
          (c) =>
            `port ${c.port} is published by container "${c.name}", which this harness did not start`,
        )
        .join("; ") +
        `. Stop it (\`docker rm -f ${foreign[0].name}\`) or free the port, then run again.`,
    );
  }

  const seen = new Set();
  for (const container of [...byName, ...byPort]) {
    if (seen.has(container.name)) continue;
    seen.add(container.name);
    if (
      container.pid &&
      container.pid !== process.pid &&
      pidAlive(container.pid)
    ) {
      throw new Error(
        `container "${container.name}" belongs to an e2e run that is still alive (pid ${container.pid}). ` +
          "Two runs cannot share mail ports; wait for it to finish.",
      );
    }
    if (!(await removeAndWait(container.name))) {
      throw new Error(
        `leftover container "${container.name}" would not go away within 30s ` +
          "(`docker ps -a` shows it); remove it by hand and run again.",
      );
    }
  }
}

/**
 * Start shard `index`'s Mailpit and return its container name.
 *
 * `onCreated` is called as soon as docker has created the container, before
 * any readiness check, because a container that started and never answered is
 * exactly the one that must still be torn down.
 */
export async function startMailpitContainer({
  index,
  smtpPort,
  apiPort,
  image,
  onCreated,
  log = () => {},
}) {
  const name = `${MAILPIT_PREFIX}${index}-${process.pid}`;
  const run = () =>
    docker([
      "run",
      "--rm",
      "-d",
      "--name",
      name,
      "--label",
      `${LABEL_PID}=${process.pid}`,
      "-p",
      `${smtpPort}:1025`,
      "-p",
      `${apiPort}:8025`,
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
    await clearLeftovers(index, [smtpPort, apiPort]);
    const result = run();
    if (result.status === 0) {
      onCreated?.(name);
      return name;
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
          ? ` — something keeps holding the name or port ${smtpPort}/${apiPort}; ` +
            `\`docker ps -a --filter publish=${apiPort}\` shows what.`
          : ""),
    );
  }
}

/** Remove containers by name, without waiting. Safe from a signal handler. */
export function removeContainers(names) {
  for (const name of names) docker(["rm", "-f", name]);
}
