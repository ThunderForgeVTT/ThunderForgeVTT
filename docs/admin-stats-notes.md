# What the admin dashboard's numbers are

Written 2026-09-15, after checking each tile against the database.

Every tile is a real reading, taken when the page loads. Five are
`count(*)` on one table (`src/server/src/admin.rs`, `load_admin_stats`), and
storage walks the data directory and adds up file sizes
(`recalculate_disk_usage`). Nothing is cached, estimated or sampled.

Verified on 2026-09-15 against the development database: users 4,168, worlds
3,115, tokens 135,904, events 153,124, policies 0 — each matching its
`count(*)` exactly.

## What each number does and does not mean

- **Users** counts every account row, including any left by tests. Test rows
  were cleared on 2026-09-15 (2,073,596 rows, when `cargo test` still wrote to
  the development database), and `cargo test` now uses `thunderforge_test`, so
  it cannot add to this again.
- **Worlds** counts every world row, including worlds nobody opens.
- **Tokens** counts every token row in the database. It is not "tokens in
  worlds being played": about 44 tokens per world here is residue from tests
  and old scenes, not a table's worth of creatures. The subtitle said "for
  active worlds", which was wrong; corrected the same day.
- **Events** counts every world event ever recorded, not a rate. It grows with
  history and is never trimmed. The subtitle said "flowing through sync", which
  read like throughput; corrected.
- **Policies** counts the `policies` table, which nothing writes yet.
  Permissions live in per-entity tables (world members, actor permissions,
  ability permissions), so this reads 0 on a healthy instance and will keep
  reading 0 until something uses that table. Consider removing the tile, or
  pointing it at the permission tables that are actually in use.
- **Storage** is the real size of the configured data root: worlds, assets,
  client files, databases and modules. It is a filesystem walk, so it costs
  something on a large instance and is taken on every load.

## Known gaps, not fixed

- **Strays are counted.** Tokens and events belonging to deleted or abandoned
  worlds are included, because the count is per table rather than joined to a
  live world. Either join them, or run a sweep for orphans, before quoting
  these numbers as a measure of use.
- **The Policies tile misleads** as described above.
- **No time dimension.** Every tile is a total since the beginning, so nothing
  says what happened this week. Whoever owns the admin portal spec (042) should
  decide whether these should be "now", "this week", or both.
