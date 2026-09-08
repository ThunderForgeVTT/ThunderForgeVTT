#!/usr/bin/env node
/**
 * Spec 043 US3 — measure OPFS blob throughput two ways, and decide.
 *
 * # Why this exists
 *
 * The whole justification for spec 043 is that synchronous access handles are
 * "markedly faster for large blobs". Nobody has measured that on this
 * codebase, at the sizes this cache actually stores, in the browsers this
 * product runs in. SC-002 sets the threshold: **at least twice the write
 * throughput at 1 MB and above, or the feature stops.**
 *
 * So this runs before the architecture, not after it, and it is built on
 * nothing that the feature would introduce — the harness in `public/bench/`
 * talks to OPFS directly. If the number says no, the only thing to throw away
 * is that harness.
 *
 * # Why it drives a real browser
 *
 * OPFS needs a secure context and the synchronous API needs a dedicated
 * worker. There is no way to measure this in Node, and a measurement taken
 * anywhere other than a real browser would be measuring something else.
 *
 * # Usage
 *
 *   pnpm dev                                    # a secure context on 127.0.0.1
 *   node scripts/bench-blob-store.mjs
 *   node scripts/bench-blob-store.mjs --browser=firefox
 *   node scripts/bench-blob-store.mjs --out=specs/043-worker-blob-store/measurements.md
 */

import { execFileSync } from "node:child_process";
import { appendFileSync, existsSync } from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";

const ROOT = new URL("..", import.meta.url).pathname;

function parseArgs(argv) {
  const args = { browser: "chromium", url: "http://127.0.0.1:5173", out: null };
  for (const arg of argv) {
    const m = /^--([a-z]+)=(.+)$/.exec(arg);
    if (!m) {
      throw new Error(
        `Unknown argument: ${arg}\n` +
          "Usage: node scripts/bench-blob-store.mjs [--browser=chromium|firefox] " +
          "[--url=http://127.0.0.1:5173] [--out=<file>]",
      );
    }
    const [, key, value] = m;
    if (!(key in args)) {
      throw new Error(`Unknown option --${key}`);
    }
    args[key] = value;
  }
  return args;
}

/** What produced a number, so that the number means something (FR-018). */
function provenance(browser, version) {
  return {
    browser: `${browser} ${version}`,
    machine: `${os.type()} ${os.release()} · ${os.cpus()[0]?.model ?? "unknown cpu"} · ${
      Math.round(os.totalmem() / 1024 ** 3)
    } GB`,
    commit: (() => {
      try {
        return execFileSync("git", ["rev-parse", "--short", "HEAD"], {
          cwd: ROOT,
        })
          .toString()
          .trim();
      } catch {
        return "unknown";
      }
    })(),
    when: new Date().toISOString(),
  };
}

const mbps = (bytesPerSecond) =>
  bytesPerSecond ? (bytesPerSecond / 1024 / 1024).toFixed(1) : "—";

async function main() {
  const args = parseArgs(process.argv.slice(2));

  // Resolved from `apps/web`, where it is a devDependency. This script lives
  // at the repository root and pnpm does not hoist it there.
  const requireFromWeb = createRequire(new URL("../apps/web/package.json", import.meta.url));
  const { chromium, firefox } = requireFromWeb("@playwright/test");
  const engines = { chromium, firefox };
  const engine = engines[args.browser];
  if (!engine) {
    throw new Error(`--browser must be one of: ${Object.keys(engines).join(", ")}`);
  }

  const browser = await engine.launch();
  const context = await browser.newContext();
  const page = await context.newPage();

  const consoleErrors = [];
  page.on("console", (msg) => {
    if (msg.type() === "error") consoleErrors.push(msg.text());
  });

  // Any page on the origin will do; the harness is a module in `public/` and
  // needs only a secure context and an origin to own an OPFS root.
  await page.goto(args.url, { waitUntil: "domcontentloaded", timeout: 60_000 });

  const rows = await page.evaluate(async () => {
    const mod = await import("/bench/blob-store-bench.js");
    return mod.run();
  });

  const meta = provenance(args.browser, browser.version());
  await browser.close();

  // Report
  const lines = [];
  lines.push("");
  lines.push(`### ${meta.browser}`);
  lines.push("");
  lines.push(`- **Machine**: ${meta.machine}`);
  lines.push(`- **Commit**: ${meta.commit} · **When**: ${meta.when}`);
  lines.push("");
  lines.push("| Size | Store | Write MB/s | Read MB/s | Reps |");
  lines.push("|---|---|---:|---:|---:|");
  for (const row of rows) {
    if (row.unavailable) {
      lines.push(`| ${row.size} | ${row.store} | — | — | unavailable: ${row.unavailable} |`);
      continue;
    }
    lines.push(
      `| ${row.size} | ${row.store} | ${mbps(row.writeBytesPerSecond)} | ${mbps(
        row.readBytesPerSecond,
      )} | ${row.repeats} |`,
    );
  }

  // SC-002's verdict, computed rather than eyeballed.
  const verdicts = [];
  for (const size of new Set(rows.map((r) => r.size))) {
    const forSize = rows.filter((r) => r.size === size);
    const async_ = forSize.find((r) => r.store === "async-main-thread");
    const sync = forSize.find((r) => r.store === "sync-in-worker");
    if (!async_?.writeBytesPerSecond || !sync?.writeBytesPerSecond) continue;
    if (async_.bytes < 1024 * 1024) continue; // SC-002 is about 1 MB and above
    verdicts.push({
      size,
      ratio: sync.writeBytesPerSecond / async_.writeBytesPerSecond,
    });
  }

  lines.push("");
  if (verdicts.length === 0) {
    lines.push(
      "**SC-002: not decidable from this run** — one of the two paths produced no figure at 1 MB or above.",
    );
  } else {
    const worst = Math.min(...verdicts.map((v) => v.ratio));
    lines.push(
      `**SC-002**: write speed-up at 1 MB and above — ${verdicts
        .map((v) => `${v.size} ×${v.ratio.toFixed(2)}`)
        .join(", ")}.`,
    );
    lines.push(
      worst >= 2
        ? `**Met** (worst case ×${worst.toFixed(2)} ≥ 2). The feature proceeds.`
        : `**Not met** (worst case ×${worst.toFixed(2)} < 2). Per the plan, the feature stops here and this measurement is the deliverable.`,
    );
  }
  if (consoleErrors.length) {
    lines.push("");
    lines.push(`_Console errors during the run: ${consoleErrors.length}_`);
  }

  const report = lines.join("\n");
  process.stdout.write(`${report}\n`);

  if (args.out) {
    if (!existsSync(args.out)) {
      throw new Error(`--out file does not exist: ${args.out}`);
    }
    appendFileSync(args.out, `${report}\n`);
    process.stdout.write(`\nAppended to ${args.out}\n`);
  }
}

main().catch((error) => {
  process.stderr.write(`${error.stack ?? error}\n`);
  process.exit(1);
});
