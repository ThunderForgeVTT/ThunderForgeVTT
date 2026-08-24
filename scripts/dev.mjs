#!/usr/bin/env node

import {
  ROOT_DIR,
  ensureEngineBuild,
  log,
  parseArgs,
  spawnManaged,
  terminateChildren,
  waitForProcess,
} from "./shared.mjs";

let shuttingDown = false;

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

  await ensureEngineBuild({ force: args.force });

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
    // A Cloudflare "quick tunnel" — no account/config needed, just an
    // ephemeral https://<random>.trycloudflare.com forwarding to the
    // Vite dev server. cloudflared logs (including the tunnel URL
    // itself) go to stderr, which spawnManaged already pipes through
    // with the "tunnel" prefix, so it just shows up in the console.
    // vite.config.mts's server.allowedHosts is what actually lets Vite
    // accept requests forwarded through that hostname.
    log("dev", "Starting cloudflared tunnel (watch for the [tunnel] line below for the URL to share)...");
    const tunnel = spawnManaged("cloudflared tunnel --url http://localhost:5173", {
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
