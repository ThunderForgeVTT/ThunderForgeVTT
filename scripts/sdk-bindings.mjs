#!/usr/bin/env node
/**
 * Regenerate the engine SDK's TypeScript types from their Rust definitions.
 *
 *   node scripts/sdk-bindings.mjs           # regenerate
 *   node scripts/sdk-bindings.mjs --check   # regenerate and fail on any diff
 *
 * # Why a check and not just a generator
 *
 * Generating types does not prevent drift; it only moves where drift can
 * happen. A generated file nobody regenerates is a hand-written file with a
 * misleading header — and the failure mode it produces is the one this whole
 * boundary exists to end: the engine ignores a field it cannot parse, so a
 * stale type shows up as a display that silently does not appear.
 *
 * So the output is committed (the web app builds without a Rust toolchain)
 * and `--check` regenerates it and fails on any difference. That is what makes
 * the commitment real rather than aspirational.
 *
 * The same shape as `schema.rs` for diesel: generated, committed, and wrong
 * loudly rather than quietly when it falls behind.
 */

import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";
import process from "node:process";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const OUT_DIR = path.join(ROOT, "apps", "web", "src", "engine", "sdk");
const check = process.argv.includes("--check");

// ts-rs emits during a test run, so the export is a `cargo test` filtered to
// the generated `export_bindings_*` cases.
//
// The destination is set on the types themselves (`#[ts(export_to = ...)]`)
// so that an ordinary `cargo test` also writes to the right place — without
// it, running the plain test suite silently created a stray `bindings/`
// directory beside the crate.
//
// `TS_RS_EXPORT_DIR` is deliberately NOT set here. `export_to` is resolved
// relative to it, so setting both compounds them and writes to
// `apps/web/apps/web/...`. One mechanism, not two.
execFileSync(
  "cargo",
  ["test", "-p", "thunderforge_canvas_core", "export_bindings"],
  { cwd: ROOT, stdio: "inherit" },
);

if (!check) {
  console.log(`[sdk] regenerated into ${path.relative(ROOT, OUT_DIR)}`);
  process.exit(0);
}

const diff = execFileSync(
  "git",
  ["status", "--porcelain", "--", path.relative(ROOT, OUT_DIR)],
  { cwd: ROOT, encoding: "utf8" },
).trim();

if (diff) {
  console.error(
    "[sdk] Generated types do not match their Rust source:\n" +
      diff +
      "\n\nRun `pnpm sdk:bindings` and commit the result.",
  );
  process.exit(1);
}

console.log("[sdk] generated types match their Rust source");
