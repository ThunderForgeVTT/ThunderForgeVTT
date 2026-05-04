#!/usr/bin/env node

import {
  ROOT_DIR,
  ensureEngineBuild,
  log,
  parseArgs,
  runCommand,
  terminateChildren,
} from "./shared.mjs";

async function run() {
  const args = parseArgs(process.argv.slice(2), { allowOnlyWasm: true });

  process.on("SIGINT", () => {
    void terminateChildren("SIGINT").then(() => process.exit(0));
  });

  process.on("SIGTERM", () => {
    void terminateChildren("SIGTERM").then(() => process.exit(0));
  });

  await ensureEngineBuild({ force: args.force });

  if (args.onlyWasm) {
    log("build", "--only-wasm set, skipping frontend/backend builds.");
    return;
  }

  log("build", "Building frontend...");
  await runCommand("pnpm -F @thunderforge/web run build", {
    name: "frontend build",
    cwd: ROOT_DIR,
    prefix: "frontend",
  });

  log("build", "Building backend...");
  await runCommand("cargo build -p thunderforge", {
    name: "backend build",
    cwd: ROOT_DIR,
    prefix: "backend",
  });

  log("build", "Build completed.");
}

run().catch(async (err) => {
  log("build", `${err.stack ?? err.message}`, process.stderr);
  await terminateChildren("SIGTERM");
  process.exit(1);
});
