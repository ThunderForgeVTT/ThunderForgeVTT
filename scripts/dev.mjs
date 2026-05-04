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
  const args = parseArgs(process.argv.slice(2), { allowOnlyWasm: false });

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

  const frontendWait = waitForProcess(frontend, "frontend");
  const backendWait = waitForProcess(backend, "backend");
  const firstExit = await Promise.race([
    frontendWait.then((result) => ({ name: "frontend", result })),
    backendWait.then((result) => ({ name: "backend", result })),
  ]);

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
