#!/usr/bin/env node

import {
  ROOT_DIR,
  ensureCombatBuild,
  ensureDiceBuild,
  engineProfile,
  ensureEngineBuild,
  ensurePdfBuild,
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

  // Release only where CI=true (CI, and the Dockerfile, which sets it): a
  // local build is a check that things compile, and a release engine is
  // minutes of wasm-opt for it. ENGINE_PROFILE still overrides either way.
  await ensureEngineBuild({
    force: args.force,
    profile: engineProfile(process.env.CI === "true" ? "release" : "dev"),
  });
  // The web imports this the same way it imports the engine, so it has to
  // exist before the frontend is built. Cheap when it already does.
  await ensurePdfBuild({ force: args.force });
  // The demo's in-page backend rolls with this (spec 074).
  await ensureDiceBuild({ force: args.force });
  // The demo's fight rules (spec 079): the demo build imports them, and the
  // image builds the demo after `--only-wasm`.
  await ensureCombatBuild({ force: args.force });

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
