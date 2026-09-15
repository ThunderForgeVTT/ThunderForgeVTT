#!/usr/bin/env node
/**
 * One-off: remove the rows `cargo test` left in the development database.
 *
 *     node scripts/cleanup-dev-test-rows.mjs            # counts only, read-only
 *     node scripts/cleanup-dev-test-rows.mjs --apply    # delete them, in one transaction
 *
 * # Why this exists
 *
 * Until `cargo test` got its own database (`thunderforge_test`, see
 * `src/server/src/test_support.rs`), every database-backed test wrote into the
 * development database and nothing took the rows away. By 2026-09-15 that was
 * about 232,000 test users and the worlds, scenes, tokens and everything else
 * hung off them. Tests no longer add to it; this removes what is there.
 *
 * It is not run by anything. It is meant to be run once, by a person, after
 * reading the dry run.
 *
 * # What counts as a test row
 *
 * A **user** whose email ends in `@example.invalid`. `.invalid` is reserved
 * (RFC 2606): no real address has it, and every test fixture that makes a user
 * uses it (`test_support::insert_test_user`, the registration and setup
 * tests). `@example.test` is deliberately *not* included — the demo seed's
 * `admin`, `user1` and `user2` live there.
 *
 * Everything else follows from the foreign keys, read from the catalog rather
 * than listed here: a row goes if it references a row that goes through a key
 * that would otherwise stop the delete (`NO ACTION`, `RESTRICT`) or would take
 * it anyway (`CASCADE`). Keys that `SET NULL` or `SET DEFAULT` keep their row.
 * A test user's **worlds** are the ones it created (`worlds.created_by`).
 *
 * Users, worlds and the instance's configuration rows are never deleted by
 * being *reached* (see `PROTECTED`): if a key leads from a test row to one of
 * them that is not itself a test row, that is reported as blocking, with the
 * fix, and `--apply` refuses.
 *
 * # Safety
 *
 * The dry run opens a `READ ONLY` transaction. `--apply` runs every delete in
 * one transaction, children before parents, and rolls back on any error — so it
 * either removes all of it or none. Stop the dev server first; a request that
 * touches a test world mid-delete would make it fail and roll back, which is
 * safe but wasted.
 */

import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import { databaseName, readDotEnv, withExplicitPort } from "./test-db.mjs";

const apply = process.argv.includes("--apply");
/**
 * Rows that are never deleted for being reached: people and worlds (only the
 * test ones go, by their own rule), and the instance's own configuration — a
 * singleton like `instance_access_settings` records who last changed it, and
 * a test that changed it must not take the instance's access policy with it.
 */
const PROTECTED = new Set([
  "users",
  "worlds",
  "instance_access_settings",
  "instance_identity",
  "instance_settings",
  "auth_security_settings",
  "admin_bootstrap_setup",
  "oauth_providers",
]);

function sqlString(value) {
  return `'${value.replaceAll("'", "''")}'`;
}

function quoteIdent(name) {
  return `"${name.replaceAll('"', '""')}"`;
}

function psql(url, sql) {
  return execFileSync(
    "psql",
    [withExplicitPort(url), "-v", "ON_ERROR_STOP=1", "-X", "-Atq"],
    { input: sql, encoding: "utf-8", maxBuffer: 64 * 1024 * 1024 },
  );
}

const FOREIGN_KEYS_SQL = `
SELECT json_agg(json_build_object(
  'name', c.conname,
  'child', cl.relname,
  'parent', pl.relname,
  'onDelete', c.confdeltype,
  'childCols', (SELECT json_agg(a.attname ORDER BY k.ord)
                FROM unnest(c.conkey) WITH ORDINALITY k(attnum, ord)
                JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = k.attnum),
  'parentCols', (SELECT json_agg(a.attname ORDER BY k.ord)
                 FROM unnest(c.confkey) WITH ORDINALITY k(attnum, ord)
                 JOIN pg_attribute a ON a.attrelid = c.confrelid AND a.attnum = k.attnum)
))
FROM pg_constraint c
JOIN pg_class cl ON cl.oid = c.conrelid
JOIN pg_class pl ON pl.oid = c.confrelid
JOIN pg_namespace n ON n.oid = cl.relnamespace
WHERE c.contype = 'f' AND n.nspname = 'public';
`;

/** The CTE holding one table's doomed rows. */
const cte = (table) => quoteIdent(`doomed_${table}`);

/**
 * Which tables lose rows, through which keys, in which order.
 *
 * Every table's doomed rows are a CTE defined from its parents' CTEs, so the
 * SQL grows with the number of keys rather than with the number of paths.
 * `order` is parents first (the order the CTEs must be written in); deletes
 * run in the reverse. A key that would close a cycle is left out and reported:
 * that can only mean fewer rows are selected, and a row the cycle still needed
 * gone makes the delete fail and roll back rather than go wrong.
 */
function plan(foreignKeys) {
  const byParent = new Map();
  for (const fk of foreignKeys) {
    if (fk.onDelete === "n" || fk.onDelete === "d") continue;
    if (!byParent.has(fk.parent)) byParent.set(fk.parent, []);
    byParent.get(fk.parent).push(fk);
  }
  const worldsKey = foreignKeys.find(
    (fk) => fk.child === "worlds" && fk.parent === "users" && fk.childCols.join() === "created_by",
  );
  if (!worldsKey) throw new Error("no worlds.created_by -> users key; the schema is not the one this was written for");

  // Discover the reachable tables and the keys into each.
  const incoming = new Map([["users", []], ["worlds", [worldsKey]]]);
  const blocking = [];
  const queue = ["users", "worlds"];
  const seen = new Set(queue);
  while (queue.length > 0) {
    const table = queue.shift();
    for (const fk of byParent.get(table) ?? []) {
      if (fk === worldsKey) continue;
      if (PROTECTED.has(fk.child) || fk.child === table) {
        blocking.push(fk);
        continue;
      }
      if (!incoming.has(fk.child)) incoming.set(fk.child, []);
      incoming.get(fk.child).push(fk);
      if (!seen.has(fk.child)) {
        seen.add(fk.child);
        queue.push(fk.child);
      }
    }
  }

  // Parents first; a key back into a table still being visited closes a cycle.
  const order = [];
  const stateOf = new Map();
  const cyclic = [];
  const visit = (table) => {
    if (stateOf.get(table) === "done") return;
    stateOf.set(table, "active");
    for (const fk of incoming.get(table)) {
      if (stateOf.get(fk.parent) === "active") {
        cyclic.push(fk);
        continue;
      }
      visit(fk.parent);
    }
    stateOf.set(table, "done");
    order.push(table);
  };
  for (const table of incoming.keys()) visit(table);
  const usable = (fk) => !cyclic.includes(fk);

  // Each key as a join, the joins as one UNION ALL of row ids: an OR of
  // EXISTS over materialised CTEs cannot be planned as a semi-join and turns
  // quadratic at a few hundred thousand rows (measured: over ten minutes).
  const definition = (table, doomed = cte) => {
    if (table === "users") return `SELECT x.ctid AS doomed_ctid, x.* FROM users x WHERE x.email LIKE '%@example.invalid'`;
    const terms = incoming
      .get(table)
      .filter(usable)
      .map((fk) => {
        const join = fk.childCols
          .map((col, i) => `p.${quoteIdent(fk.parentCols[i])} = y.${quoteIdent(col)}`)
          .join(" AND ");
        return `SELECT y.ctid FROM ${quoteIdent(table)} y JOIN ${doomed(fk.parent)} p ON ${join}`;
      });
    if (terms.length === 0) return `SELECT x.ctid AS doomed_ctid, x.* FROM ${quoteIdent(table)} x WHERE FALSE`;
    return `SELECT x.ctid AS doomed_ctid, x.* FROM ${quoteIdent(table)} x WHERE x.ctid IN (${terms.join(" UNION ALL ")})`;
  };
  const withClause = `WITH ${order.map((t) => `${cte(t)} AS (${definition(t)})`).join(",\n")}`;

  // Rows outside the test set that a test row reaches through a key into
  // users, worlds, or back into its own table.
  const blockingSql = blocking.map((fk) => {
    const join = fk.childCols
      .map((col, i) => `p.${quoteIdent(fk.parentCols[i])} = x.${quoteIdent(col)}`)
      .join(" AND ");
    const own = incoming.has(fk.child)
      ? `NOT EXISTS (SELECT 1 FROM ${cte(fk.child)} o WHERE o.doomed_ctid = x.ctid)`
      : "TRUE";
    return {
      via: `${fk.child}.${fk.childCols.join(",")} -> ${fk.parent}`,
      sql: `SELECT count(*) FROM ${quoteIdent(fk.child)} x WHERE EXISTS (SELECT 1 FROM ${cte(fk.parent)} p WHERE ${join}) AND ${own}`,
    };
  });

  return { order, withClause, blockingSql, cyclic, definition, incoming };
}

function main() {
  const env = { ...readDotEnv(), ...process.env };
  const url = env.DATABASE_URL;
  if (!url) throw new Error("DATABASE_URL is not set (and no .env was found)");
  const name = databaseName(url);
  console.log(`cleanup-dev-test-rows: ${name} (${apply ? "APPLY" : "read-only counts"})`);

  const foreignKeys = JSON.parse(psql(url, FOREIGN_KEYS_SQL).trim() || "[]");
  const { order, withClause, blockingSql, cyclic, definition } = plan(foreignKeys);
  for (const fk of cyclic) {
    console.log(`  note: ${fk.child}.${fk.childCols.join(",")} -> ${fk.parent} closes a cycle and is not followed`);
  }

  const started = performance.now();
  const countSql = [
    "BEGIN TRANSACTION READ ONLY;",
    "SET LOCAL statement_timeout = 0;",
    `${withClause}\nSELECT ${order.map((t) => `'${t}=' || (SELECT count(*) FROM ${cte(t)})`).join(" || ';' || ")};`,
    ...blockingSql.map((b, i) => `${withClause}\nSELECT 'blocking:${i}=' || (${b.sql});`),
    "SELECT 'total users=' || count(*) FROM users;",
    "SELECT 'total worlds=' || count(*) FROM worlds;",
    "ROLLBACK;",
  ].join("\n");
  const counts = new Map(
    psql(url, countSql)
      .trim()
      .split(/[\n;]/)
      .filter((part) => part.includes("="))
      .map((part) => {
        const at = part.lastIndexOf("=");
        return [part.slice(0, at), Number(part.slice(at + 1))];
      }),
  );

  console.log(`\nwould remove (${Math.round(performance.now() - started)} ms to count):`);
  const rows = order.filter((t) => counts.get(t) > 0).sort((a, b) => counts.get(b) - counts.get(a));
  const width = Math.max(...rows.map((t) => t.length), 10);
  for (const table of rows) console.log(`  ${table.padEnd(width)}  ${counts.get(table).toLocaleString("en-US")}`);
  const total = rows.reduce((sum, t) => sum + counts.get(t), 0);
  console.log(`  ${"(total)".padEnd(width)}  ${total.toLocaleString("en-US")} rows across ${rows.length} tables`);
  console.log(
    `\nof ${counts.get("total users").toLocaleString("en-US")} users and ${counts.get("total worlds").toLocaleString("en-US")} worlds in ${name}, ` +
      `${(counts.get("users") ?? 0).toLocaleString("en-US")} users and ${(counts.get("worlds") ?? 0).toLocaleString("en-US")} worlds are test rows.`,
  );

  const blocked = blockingSql.filter((_, i) => counts.get(`blocking:${i}`) > 0);
  if (blocked.length > 0) {
    console.log("\nblocking — rows outside the test set that a test row reaches:");
    for (const b of blocked) console.log(`  ${b.via}: ${counts.get(`blocking:${blockingSql.indexOf(b)}`)}`);
    console.log(
      "  Point those rows at a real account first (for a singleton, e.g.\n" +
        "  UPDATE instance_access_settings SET updated_by = (SELECT id FROM users WHERE username = 'admin');),\n" +
        "  then run this again.",
    );
  }

  if (!apply) {
    console.log("\nnothing was changed. --apply deletes the rows above in one transaction.");
    return 0;
  }
  if (blocked.length > 0) {
    console.error("\nrefusing to apply while anything is blocking.");
    return 1;
  }
  // Measured on a copy of the development database: deriving each delete's
  // rows from a chain of CTEs, with every foreign-key trigger firing per row,
  // had not finished the first table after ten minutes. So the doomed rows are
  // written down once, into temporary tables, and the deletes run with
  // triggers off (`session_replication_role = replica`) — which is only sound
  // because this deletes the whole closure itself, nulls what `SET NULL` keys
  // would have nulled, and then *checks* every key into a table it touched
  // before committing. An orphan anywhere raises, and the transaction rolls
  // back.
  const temp = (table) => quoteIdent(`doomed_${table}`);
  const touched = new Set(order);
  const nulling = foreignKeys.filter((fk) => touched.has(fk.parent) && (fk.onDelete === "n" || fk.onDelete === "d"));
  const checked = foreignKeys.filter((fk) => touched.has(fk.parent));
  const keyMatch = (fk, child, parent) =>
    fk.childCols.map((col, i) => `${parent}.${quoteIdent(fk.parentCols[i])} = ${child}.${quoteIdent(col)}`).join(" AND ");
  const deleteSql = [
    "\\set ON_ERROR_STOP 1",
    "BEGIN;",
    "SET LOCAL statement_timeout = 0;",
    ...order.flatMap((table) => [
      `CREATE TEMP TABLE ${temp(table)} ON COMMIT DROP AS ${definition(table, temp)};`,
      `ANALYZE ${temp(table)};`,
    ]),
    "SET LOCAL session_replication_role = replica;",
    ...[...order].reverse().map(
      (table) => `DELETE FROM ${quoteIdent(table)} d USING ${temp(table)} x WHERE d.ctid = x.doomed_ctid;`,
    ),
    // After the deletes, not before: an UPDATE gives a row a new ctid, and a
    // doomed row that a nulling touched first would no longer match its entry.
    ...nulling.map(
      (fk) =>
        `UPDATE ${quoteIdent(fk.child)} c SET ${fk.childCols
          .map((col) => `${quoteIdent(col)} = ${fk.onDelete === "n" ? "NULL" : "DEFAULT"}`)
          .join(", ")} FROM ${temp(fk.parent)} p WHERE ${keyMatch(fk, "c", "p")};`,
    ),
    "SET LOCAL session_replication_role = origin;",
    "DO $check$ BEGIN",
    ...checked.map(
      (fk) =>
        `  IF EXISTS (SELECT 1 FROM ${quoteIdent(fk.child)} c WHERE ${fk.childCols
          .map((col) => `c.${quoteIdent(col)} IS NOT NULL`)
          .join(" AND ")} AND NOT EXISTS (SELECT 1 FROM ${quoteIdent(fk.parent)} p WHERE ${keyMatch(fk, "c", "p")})) THEN\n` +
        `    RAISE EXCEPTION 'cleanup would orphan rows through %; nothing was deleted', ${sqlString(fk.name)};\n  END IF;`,
    ),
    "END $check$;",
    "COMMIT;",
  ].join("\n");
  const applied = performance.now();
  execFileSync("psql", [withExplicitPort(url), "-v", "ON_ERROR_STOP=1", "-X", "-q"], {
    input: deleteSql,
    stdio: ["pipe", "inherit", "inherit"],
  });
  console.log(`\ndeleted, in ${Math.round((performance.now() - applied) / 1000)} s.`);
  return 0;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    process.exit(main());
  } catch (error) {
    console.error(`cleanup-dev-test-rows: ${error.message}`);
    process.exit(1);
  }
}
