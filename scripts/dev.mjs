#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  readlinkSync,
  symlinkSync,
  unlinkSync,
} from "node:fs";
import path from "node:path";

import {
  ROOT_DIR,
  ensureEngineBuild,
  engineProfile,
  log,
  parseArgs,
  spawnManaged,
  terminateChildren,
  waitForProcess,
} from "./shared.mjs";

let shuttingDown = false;

/**
 * Where each layer answers a readiness probe.
 *
 * The backend's health routes are registered on `api_router`, which
 * `main.rs` nests under `/api` — so they live at `/api/readyz`, not
 * `/readyz`. Getting that wrong is silent in the worst way: the probe reads
 * a 404 as "not ready yet" and waits out its whole timeout before blaming
 * a backend that was up the entire time.
 */
const BACKEND_URL = "http://127.0.0.1:30000/api";
const FRONTEND_URL = "http://127.0.0.1:5173";

/**
 * Wait for one layer to report itself ready before starting the next.
 *
 * The dev stack used to start everything at once and hope. That is fine
 * until something is slow or already running, and then the error that
 * reaches the console belongs to whichever process noticed first rather
 * than to the one that actually failed — a stale server holding a port
 * surfaced as the *backend* panicking two screens after Vite had quietly
 * moved to another port.
 *
 * So each layer is gated on the same signal an orchestrator would use:
 * postgres on `pg_isready` (in the Makefile), the backend on its own
 * `/readyz`, which checks the database rather than merely that the process
 * is listening, and the frontend on answering an HTTP request at all. The
 * tunnel goes last, because a tunnel to a server that is not up yet is the
 * one failure that looks like a broken tunnel.
 *
 * `child` is watched while waiting: a process that exits during startup is
 * reported as itself, immediately, instead of being waited out for the full
 * timeout and blamed on a readiness check.
 */
async function waitUntilReady(name, url, child, timeoutMs = 180_000) {
  const deadline = Date.now() + timeoutMs;
  let exited = false;
  child.once("exit", () => {
    exited = true;
  });

  while (Date.now() < deadline) {
    if (shuttingDown) return false;
    if (exited) {
      log("dev", `${name} exited before it became ready.`, process.stderr);
      return false;
    }
    try {
      const response = await fetch(url, { signal: AbortSignal.timeout(2_000) });
      if (response.ok) {
        log("dev", `${name} is ready.`);
        return true;
      }
    } catch {
      // Not up yet. Connection refused is the expected answer for most of
      // this loop, so it is not worth logging until the deadline passes.
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }

  log(
    "dev",
    `${name} did not become ready within ${Math.round(timeoutMs / 1000)}s (${url}).`,
    process.stderr,
  );
  return false;
}

/**
 * The two tunnel keys from the repo-root `.env`, if it has them.
 *
 * Deliberately not a general dotenv loader: everything else that needs the
 * file either reads it itself (the server, via dotenvy) or is handed it by
 * the Makefile. This exists so that `pnpm dev --tunnel` and
 * `make dev-tunnel` behave the same, because the alternative is a silent
 * downgrade to an ephemeral URL with nothing on screen to explain it.
 *
 * Values are read as written — no quote stripping beyond a surrounding pair,
 * no interpolation. A token that needs more parsing than this is a token
 * that should be exported by the shell.
 */
function readTunnelEnv() {
  const found = {};
  try {
    const text = readFileSync(path.join(ROOT_DIR, ".env"), "utf8");
    for (const line of text.split("\n")) {
      const match = /^\s*(TUNNEL_TOKEN|TUNNEL_HOSTNAME)\s*=\s*(.*)$/.exec(line);
      if (!match) continue;
      found[match[1]] = match[2].trim().replace(/^(['"])(.*)\1$/, "$2");
    }
  } catch {
    // No .env, or unreadable. Not an error: the environment may carry these
    // already, and if it does not the quick-tunnel path says so out loud.
  }
  return found;
}

/**
 * Reclaim the ports this stack is about to bind, if a previous run left
 * something on them.
 *
 * A leftover dev server is the single most common way to start the day, and
 * the failure it produces is disproportionate: Vite refuses to bind, the
 * backend refuses to bind, and the message that reaches the console belongs
 * to whichever noticed first. Clearing the way is better than explaining it.
 *
 * **It will only kill something it can identify as ours.** A process holding
 * 5173 that is not this repository's dev server is somebody else's work, and
 * killing it would be a far worse failure than the one being fixed — so that
 * case reports what is there and stops, rather than guessing. The check is
 * the process's own working directory: our children run from this repo.
 *
 * SIGTERM first, and SIGKILL only for what is still there afterwards.
 */
async function reclaimPorts(ports) {
  for (const { port, what } of ports) {
    let pids = [];
    try {
      const listed = execFileSync("lsof", ["-ti", `tcp:${port}`, "-sTCP:LISTEN"], {
        encoding: "utf8",
      });
      pids = listed.split("\n").map((line) => line.trim()).filter(Boolean);
    } catch {
      // lsof exits non-zero when nothing is listening, which is the good case.
      continue;
    }

    for (const pid of pids) {
      let owner = "";
      try {
        owner = readlinkSync(`/proc/${pid}/cwd`);
      } catch {
        // A process we cannot inspect is one we have no business killing.
      }

      if (!owner.startsWith(ROOT_DIR)) {
        log(
          "dev",
          `Port ${port} (${what}) is held by pid ${pid}, which is not running from this ` +
            `repository${owner ? ` (cwd: ${owner})` : ""}. Leaving it alone — stop it yourself ` +
            "if it is safe to, then start again.",
          process.stderr,
        );
        process.exit(1);
      }

      log("dev", `Reclaiming port ${port} (${what}) from a previous run, pid ${pid}.`);
      try {
        process.kill(Number(pid), "SIGTERM");
      } catch {
        continue;
      }
    }
  }

  // One grace period for everything, then insist. A process that ignores
  // SIGTERM still has to let go of the port, or the next line fails for the
  // same reason as before.
  if (ports.length > 0) {
    await new Promise((resolve) => setTimeout(resolve, 1_500));
    for (const { port } of ports) {
      try {
        const listed = execFileSync("lsof", ["-ti", `tcp:${port}`, "-sTCP:LISTEN"], {
          encoding: "utf8",
        });
        for (const pid of listed.split("\n").map((l) => l.trim()).filter(Boolean)) {
          let owner = "";
          try {
            owner = readlinkSync(`/proc/${pid}/cwd`);
          } catch {
            continue;
          }
          if (!owner.startsWith(ROOT_DIR)) continue;
          log("dev", `pid ${pid} did not stop on SIGTERM; sending SIGKILL.`);
          process.kill(Number(pid), "SIGKILL");
        }
      } catch {
        // Nothing left listening. That is the whole point.
      }
    }
  }
}

/**
 * Give the dev stack the packs that are in the repo, every start.
 *
 * `config/mod.rs` resolves `systems_dir` as `<data path>/packs/systems` and
 * `interface_packs_dir` as `<data path>/packs/interface`, and since spec 032
 * T085 the server *lists those directories* to answer which systems exist.
 * So a pack that is not linked here is a pack the product does not have.
 *
 * # Why this exists
 *
 * It did not, and the omission was invisible. `data/packs/systems/` was a
 * farm of symlinks made by hand on one afternoon in August, and nothing kept
 * it in step with `packs/systems/`. Adding a system pack to the repo and
 * starting the dev stack simply did not offer it — no error, no warning, just
 * a picker that did not list the thing you had just written. That is another
 * hand-kept list of systems, in a costume that made it hard to see as one.
 *
 * `scripts/e2e-parallel.mjs` already got this right: it derives the links
 * from the directory on every run. This is the same loop, for the same
 * reason, so that SC-004's "adding a system touches only that system's own
 * pack directory" is true of a dev machine and not only of CI.
 *
 * Symlinks rather than copies: the shipping system packs are 110MB.
 */
function linkPacks(dataPath) {
  for (const kind of ["systems", "interface"]) {
    const source = path.join(ROOT_DIR, "packs", kind);
    if (!existsSync(source)) continue;
    const target = path.join(dataPath, "packs", kind);
    mkdirSync(target, { recursive: true });

    const wanted = readdirSync(source, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => entry.name);

    // A link to a pack that has been renamed or removed points at nothing,
    // and a dangling entry is worse than a missing one: it is a pack the
    // server tries to read and cannot.
    for (const entry of readdirSync(target)) {
      const link = path.join(target, entry);
      if (!lstatSync(link).isSymbolicLink()) continue;
      if (!wanted.includes(entry) || !existsSync(link)) unlinkSync(link);
    }

    let linked = 0;
    for (const name of wanted) {
      const link = path.join(target, name);
      if (existsSync(link)) continue;
      symlinkSync(path.join(source, name), link, "dir");
      linked += 1;
    }
    if (linked > 0) {
      log("dev", `Linked ${linked} new ${kind} pack(s) into the data directory.`);
    }
  }
}

async function run() {
  const args = parseArgs(process.argv.slice(2), { allowOnlyWasm: false, allowTunnel: true });

  const handleSignal = async (signal) => {
    shuttingDown = true;
    await terminateChildren(signal);
    process.exit(0);
  };

  process.on("SIGINT", () => {
    void handleSignal("SIGINT");
  });

  process.on("SIGTERM", () => {
    void handleSignal("SIGTERM");
  });

  process.on("uncaughtException", async (err) => {
    log(
      "dev",
      `Uncaught exception: ${err.stack ?? err.message}`,
      process.stderr
    );
    await terminateChildren("SIGTERM");
    process.exit(1);
  });

  process.on("unhandledRejection", async (reason) => {
    log("dev", `Unhandled rejection: ${String(reason)}`, process.stderr);
    await terminateChildren("SIGTERM");
    process.exit(1);
  });

  // The dev loop defaults to the fast profile: a seven-minute engine build
  // after every edit is not a dev loop. Set ENGINE_PROFILE=release when you
  // specifically want to look at load performance.
  // Before the engine build, so a stale stack is cleared while the build is
  // the thing taking time rather than after it.
  await reclaimPorts([
    { port: 5173, what: "frontend" },
    { port: 30000, what: "backend" },
  ]);

  // Before the backend starts, because the backend reads these directories
  // to answer which systems and interface packs exist.
  linkPacks(process.env.THUNDERFORGE_DATA_PATH ?? path.join(ROOT_DIR, "data"));

  await ensureEngineBuild({ force: args.force, profile: engineProfile("dev") });

  // Started in dependency order, each gated on the next being ready — the
  // database is already waited for by the Makefile's `services-up`.
  log("dev", "Starting backend...");
  const backend = spawnManaged("cargo run -p thunderforge", {
    cwd: ROOT_DIR,
    prefix: "backend",
  });

  const waits = [
    waitForProcess(backend, "backend").then((result) => ({ name: "backend", result })),
  ];

  // `/readyz` rather than `/healthz`: liveness only says the process is
  // listening, while readiness says it can reach the database, which is what
  // the frontend actually needs from it.
  if (!(await waitUntilReady("backend", `${BACKEND_URL}/readyz`, backend))) {
    await terminateChildren("SIGTERM");
    process.exit(1);
  }

  log("dev", "Starting frontend...");
  const frontend = spawnManaged("pnpm -F @thunderforge/web run dev", {
    cwd: ROOT_DIR,
    prefix: "frontend",
  });
  waits.push(
    waitForProcess(frontend, "frontend").then((result) => ({ name: "frontend", result })),
  );

  if (!(await waitUntilReady("frontend", FRONTEND_URL, frontend))) {
    await terminateChildren("SIGTERM");
    process.exit(1);
  }

  if (args.tunnel) {
    // Two kinds of tunnel, and which one you get depends on whether
    // TUNNEL_TOKEN is set.
    //
    // **Named tunnel (TUNNEL_TOKEN set).** A tunnel you created in the
    // Cloudflare dashboard, with a hostname that does not change between
    // runs. That is the difference that matters for a play test: people can
    // bookmark the URL, and a restart does not invalidate the link you sent
    // them. Where each hostname routes is configured on Cloudflare's side,
    // so nothing about it lives here except the credential.
    //
    // **Quick tunnel (no token).** An ephemeral
    // https://<random>.trycloudflare.com, needing no account. Convenient for
    // a one-off, and the reason this is the fallback rather than the default:
    // the URL changes every run, and the service is rate-limited and refuses
    // connections often enough that "the tunnel is broken" is usually this.
    //
    // Either way `vite.config.mts`'s `server.allowedHosts` is what lets Vite
    // accept requests arriving through the tunnel's hostname — a named
    // tunnel needs TUNNEL_HOSTNAME set too, or Vite answers 403 to a tunnel
    // that is working perfectly.
    // Read from `.env` when the environment does not already carry them.
    // `make dev-tunnel` exports the file, but `pnpm dev --tunnel` does not,
    // and the difference is invisible: the tunnel silently falls back to an
    // ephemeral URL and the person running it has no idea why. Only these
    // two keys are read, and only as a fallback.
    const fromEnvFile = readTunnelEnv();
    const token = (process.env.TUNNEL_TOKEN ?? fromEnvFile.TUNNEL_TOKEN)?.trim();
    const hostname = (
      process.env.TUNNEL_HOSTNAME ?? fromEnvFile.TUNNEL_HOSTNAME
    )?.trim();

    let command;
    if (token) {
      if (!hostname) {
        log(
          "dev",
          "TUNNEL_TOKEN is set but TUNNEL_HOSTNAME is not — Vite will reject requests " +
            "arriving through the tunnel with a 403. Set TUNNEL_HOSTNAME to the hostname " +
            "you configured for this tunnel.",
        );
      }
      log(
        "dev",
        hostname
          ? `Starting named cloudflared tunnel at https://${hostname}`
          : "Starting named cloudflared tunnel",
      );
      // The token goes through the environment, not the command line.
      // cloudflared reads TUNNEL_TOKEN itself, and an argument would be
      // visible in `ps` to every other user on the machine — a credential
      // that grants ingress to this tunnel has no business being there.
      process.env.TUNNEL_TOKEN = token;
      // `env -u TUNNEL_HOSTNAME` because `TUNNEL_*` is cloudflared's own
      // configuration namespace, and it reads TUNNEL_HOSTNAME as its
      // `hostname` property. Left in place it logs "The property `hostname`
      // in your configuration is ignored because you configured a Named
      // Tunnel" on every start — noise that reads like a misconfiguration
      // when nothing is wrong. Vite still needs the value for
      // `server.allowedHosts`, so it is unset for this child only rather
      // than renamed, which keeps one name in `.env` for one idea.
      command = "env -u TUNNEL_HOSTNAME cloudflared tunnel run";
    } else {
      log(
        "dev",
        "No TUNNEL_TOKEN set — starting an ephemeral quick tunnel. The URL changes every " +
          "run and the service is often rate-limited; set TUNNEL_TOKEN in .env for a stable one. " +
          "Watch for the [tunnel] line below for the URL to share.",
      );
      command = "cloudflared tunnel --url http://localhost:5173";
    }

    const tunnel = spawnManaged(command, {
      cwd: ROOT_DIR,
      prefix: "tunnel",
    });
    waits.push(waitForProcess(tunnel, "tunnel").then((result) => ({ name: "tunnel", result })));
  }

  const firstExit = await Promise.race(waits);

  if (!shuttingDown) {
    const { name, result } = firstExit;
    if (result.signal) {
      log(
        "dev",
        `${name} exited due to ${result.signal}. Stopping remaining process...`
      );
    } else if (result.code !== 0) {
      log(
        "dev",
        `${name} exited with code ${result.code}. Stopping remaining process...`,
        process.stderr
      );
    } else {
      log("dev", `${name} exited normally. Stopping remaining process...`);
    }

    await terminateChildren("SIGTERM");
    process.exit(result.code || 0);
  }
}

run().catch(async (err) => {
  log("dev", `${err.stack ?? err.message}`, process.stderr);
  await terminateChildren("SIGTERM");
  process.exit(1);
});
