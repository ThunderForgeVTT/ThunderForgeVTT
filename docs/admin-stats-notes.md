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

# The admin screens, read by their owner (2026-09-15)

A walk through the admin portal, with what each screen turned out to be, what
was wrong, and what was decided. Presentation fixes are being made; anything
needing server work or a spec is recorded here and queued.

## Storage (`/admin/storage`) — the numbers are real but measure the wrong thing

`recalculate_disk_usage` walks the configured data root. That is honest about
files, and misleading as "how big is this instance":

- `data/client` 263 MB and `data/assets` 51 MB make up nearly all of the
  312 MB shown.
- **Postgres is not counted at all.** On this machine it is 672 MB, twice the
  reported total.
- **The object store is not counted.** Uploaded images live in RustFS/S3, not
  under the data root.
- `worlds`, `modules` and `databases` read 0 bytes because they are empty
  directories left from the pre-database design. They are not a measurement.

**Decided (owner, 2026-09-15):** measure the real three — database size,
object-store size, files on disk — and drop the vestigial rows. That needs a
server change, so it is queued as its own task rather than done here.

## Security (`/admin/security`) — true, but it does not explain itself

The enforcement switch and the enrolled / not-enrolled counts are the useful
parts, and the screen says little about what turning it on does to someone
without a second factor. The bootstrap record gives dates with no reason to
care.

**Decided (owner, 2026-09-15):** an operator should be able to *ask* people to
enrol, not only require it — a skippable prompt at sign-in and a dismissible
banner in the profile, with the admin screen showing enrolment progress.
Nobody is locked out. Queued as its own task; the presentation pass only makes
room for it.

## Access (`/admin/access`) — a spec-sized decision

The screen chooses open, invite-only or closed. The owner's reading: **a closed
or invite-only instance has no real sharing, so it should not have to carry
takedown information; only an open instance does.** Instances should start
closed and be opened deliberately.

**Decided (owner, 2026-09-15):** write it as its own spec, and amend the
constitution's DMCA guardrail (spec 015, ADR-043) in the same change, because
that guardrail currently assumes sharing is always possible. New instances
default to closed; existing instances keep their current mode, so nobody's
table goes dark on an upgrade. Queued.

## Legal (`/admin/legal`) — it works, it just looks inert

The tabs do refetch by kind. With no enquiries every tab looks identical, which
reads as broken. It is the legal inbox; copyright takedowns are worked in the
moderation queue. Being fixed in presentation: counts beside each tab, an empty
state per tab naming what would appear there, and the moderation link made
prominent.

## Configuration, instance, readiness, mail — being redrawn

- **Configuration:** stacked cards becoming tables.
- **Instance:** every declared setting in one scroll, becoming one group at a
  time with the group in the URL.
- **Readiness:** each unmet capability gains a link to the setting that fixes
  it; a setting fixed by an environment variable says so instead.
- **Mail:** the SMTP settings move onto the page with the tester. They are
  already `Backing::Row` in the settings registry, so the database is their
  home and the environment is only an override — the same is true of
  `operator.name`, `operator.contact_email` and `operator.jurisdiction`. The
  gap was never storage; it was that nothing offered them.
