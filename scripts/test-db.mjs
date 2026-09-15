#!/usr/bin/env node
/**
 * The database `cargo test` uses, from the command line.
 *
 *     node scripts/test-db.mjs url     # which database tests will use
 *     node scripts/test-db.mjs reset   # drop it, create it, migrate it
 *
 * # Why this exists
 *
 * Tests have their own database, `thunderforge_test` by default
 * (`src/server/src/test_support.rs` has the rule and the reasons). The test
 * harness creates and migrates it on demand, so this is only needed to start
 * clean: after a migration was edited in place, or to shed the rows every run
 * leaves behind. `make test-db-reset` runs it.
 *
 * The URL is chosen exactly as the harness chooses it — `TEST_DATABASE_URL`,
 * else `DATABASE_URL` with the database renamed, else the local default — and
 * the same names are refused: the development database and the e2e shards'.
 * The rebuild follows `scripts/e2e-parallel.mjs`'s template: drop with
 * `FORCE`, create, `diesel migration run`.
 */

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(fileURLToPath(import.meta.url), "..", "..");
export const TEST_DATABASE_NAME = "thunderforge_test";

/**
 * `.env` values, without putting them in this process's environment. Found
 * the way `dotenvy` finds it — here or in the nearest parent — so a worktree
 * under `.claude/worktrees/` reads the checkout's, as its tests do.
 */
export function readDotEnv(root = ROOT) {
  let dir = root;
  while (!existsSync(join(dir, ".env")) && dirname(dir) !== dir) dir = dirname(dir);
  const path = join(dir, ".env");
  if (!existsSync(path)) return {};
  const values = {};
  for (const line of readFileSync(path, "utf-8").split("\n")) {
    const match = /^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*?)\s*$/.exec(line);
    if (!match) continue;
    values[match[1]] = match[2].replace(/^(['"])(.*)\1$/, "$2");
  }
  return values;
}

export function databaseName(url) {
  const afterScheme = url.includes("://") ? url.split("://")[1] : url;
  const slash = afterScheme.indexOf("/");
  if (slash < 0) return "";
  return afterScheme.slice(slash + 1).split(/[?#]/)[0];
}

export function withDatabaseName(url, name) {
  const [scheme, afterScheme] = url.includes("://") ? url.split("://") : ["postgres", url];
  const slash = afterScheme.indexOf("/");
  const authority = slash < 0 ? afterScheme : afterScheme.slice(0, slash);
  const path = slash < 0 ? "" : afterScheme.slice(slash + 1);
  const query = path.search(/[?#]/) >= 0 ? path.slice(path.search(/[?#]/)) : "";
  return `${scheme}://${authority}/${name}${query}`;
}

/**
 * A URL with no port means 5432 to diesel and the server, but a `psql` from a
 * multi-cluster install may default elsewhere (the Makefile's `seed` target
 * has the story). Name it, so this talks to the same server the tests do.
 */
export function withExplicitPort(url) {
  return url.replace(/@([^:/@]+)\//, "@$1:5432/");
}

/** Mirrors `test_support::choose_test_database_url`. Throws when refused. */
export function chooseTestDatabaseUrl(env = { ...readDotEnv(), ...process.env }) {
  const explicit = env.TEST_DATABASE_URL?.trim();
  const dev = env.DATABASE_URL?.trim();
  const chosen = explicit
    ? explicit
    : dev
      ? withDatabaseName(dev, TEST_DATABASE_NAME)
      : `postgres://postgres:password@localhost:5432/${TEST_DATABASE_NAME}`;
  const name = databaseName(chosen);
  if (
    !name ||
    name === "thunderforge" ||
    name.startsWith("thunderforge_e2e") ||
    (explicit && dev && name === databaseName(dev))
  ) {
    throw new Error(
      `refusing to use the database \`${name}\` for tests: it is the development database or an e2e shard's`,
    );
  }
  if (!/^[A-Za-z0-9_]+$/.test(name)) {
    throw new Error(`the test database name \`${name}\` must be letters, digits and underscores`);
  }
  return chosen;
}

function psql(url, sql) {
  return execFileSync("psql", [withExplicitPort(url), "-v", "ON_ERROR_STOP=1", "-Atq", "-c", sql], {
    encoding: "utf-8",
  });
}

function reset() {
  const url = chooseTestDatabaseUrl();
  const name = databaseName(url);
  const maintenance = withDatabaseName(url, "postgres");
  const started = performance.now();
  console.log(`test-db: dropping and rebuilding ${name}`);
  psql(maintenance, `DROP DATABASE IF EXISTS "${name}" WITH (FORCE);`);
  psql(maintenance, `CREATE DATABASE "${name}";`);
  execFileSync("diesel", ["migration", "run"], {
    cwd: join(ROOT, "src/server"),
    env: { ...process.env, DATABASE_URL: withExplicitPort(url) },
    stdio: ["ignore", "ignore", "inherit"],
  });
  const migrations = psql(url, "SELECT count(*) FROM __diesel_schema_migrations;").trim();
  console.log(
    `test-db: ${name} is empty and migrated (${migrations} migrations, ${Math.round(performance.now() - started)} ms)`,
  );
}

function main() {
  const [command] = process.argv.slice(2);
  try {
    if (command === "url") console.log(chooseTestDatabaseUrl());
    else if (command === "reset") reset();
    else {
      console.error("usage: node scripts/test-db.mjs url|reset");
      return 2;
    }
  } catch (error) {
    console.error(`test-db: ${error.message}`);
    return 1;
  }
  return 0;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  process.exit(main());
}
