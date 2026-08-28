#!/usr/bin/env node

import { readFileSync } from "node:fs";
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
  await ensureEngineBuild({ force: args.force, profile: engineProfile("dev") });

  log("dev", "Starting frontend and backend...");
  const frontend = spawnManaged("pnpm -F @thunderforge/web run dev", {
    cwd: ROOT_DIR,
    prefix: "frontend",
  });
  const backend = spawnManaged("cargo run -p thunderforge", {
    cwd: ROOT_DIR,
    prefix: "backend",
  });

  const waits = [
    waitForProcess(frontend, "frontend").then((result) => ({ name: "frontend", result })),
    waitForProcess(backend, "backend").then((result) => ({ name: "backend", result })),
  ];

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
