# Game Systems DB Model and Ownership Rules

- **Date**: 2026-05-04 (opened) / 2026-09-03 (decided)
- **Status**: Accepted
- **Spec**: `specs/032-pack-architecture/` (FR-005, FR-029, SC-004), Increment F
- **Related**: ADR-029 (outside code is not executed), ADR-061 (a pack's
  implementation is discovered, not listed)

## Context

This file was opened on 2026-05-04 and stayed empty. Spec 032's Increment F
made it a live question rather than a tidy-up, because the increment's first
task could not be done without answering it:

> What is the `game_systems` table **for**, if `packs/systems/` is the source
> of truth for which systems exist?

### What was actually there

`/api/systems` read the `game_systems` table. That table held **zero rows** —
measured against the development database on 2026-09-03, not assumed. Nothing
has ever seeded it with the bundled packs.

So the server honestly answered an empty list, and the client compensated.
`apps/web/src/api/gameSystems.ts` carried `BUNDLED_SYSTEM_IDS` and
`BUNDLED_SYSTEM_LABELS`: two hand-kept literals naming all seven bundled
systems and their titles, in shared web code. Its own comment admitted the
shape of the problem — *"there is no reconciled installed system packs catalog
yet ... extend it (or replace it with a real catalog query) as more packs are
added."*

That is SC-004's violation standing in the open: adding a system was supposed
to touch only that system's own pack directory, and it demanded an edit there
as well.

### The asymmetry that gave it away

The interface-pack half had already solved this and solved it differently.
`interface_packs::list_installed` reads a directory, and the client's comment
recorded the consequence — *"unlike `BUNDLED_SYSTEM_IDS` ... there is no
hand-kept list here."* Two halves of one feature, one reading a directory and
one reading an empty table, and only one of them needed a literal to work.

The asymmetry was the bug. Neither half is more complicated than the other.

## Decision

**The directory is the row of record for which game systems exist. The
`game_systems` table is not, and never was.**

1. `/api/systems` lists `packs/systems/` — reading each `system.json` for its
   `id`, `title`, `description` and `version`, in title order. A pack that
   fails to parse is omitted rather than listed, because offering a Game
   Master something that cannot be chosen is worse than not offering it.
2. A pack may declare `"template": true` and is then not offered as a system a
   world can be bound to. This is a **declaration by the pack**, never a name
   in shared code — `basic-game-system` is the pack that declares it. Shared
   code omitting a pack by name would put back precisely the hardcoded
   knowledge this decision removes.
3. `BUNDLED_SYSTEM_IDS` and `BUNDLED_SYSTEM_LABELS` are deleted.
4. The `gameSystems` GraphQL query and its `load_game_systems` loader are
   deleted. Nothing read them, and two code paths answering "which systems
   exist" from two different stores is the asymmetry above with a second
   address.
5. **The table is kept, and its meaning is narrowed.** It records systems
   installed at runtime through the admin upload flow
   (`POST /api/systems/install`), which writes a row and unpacks the archive
   into the systems directory. It is a record of *installations performed*,
   not a catalogue of *systems available*. Those coincide only when every
   system arrived that way, and for a bundled pack none did: a bundled pack
   ships, and shipping is not an installation.

### Why not seed the table from the directory at boot

Considered and rejected. It makes the database a cache of the filesystem with
the filesystem still authoritative, which adds a staleness mode and buys
nothing: every read would still have to be correct when the cache is empty,
because the cache is empty on every fresh install. A row per installed system
earns its place when a system can arrive at runtime — and per ADR-029, a
third-party system pack cannot arrive at all today.

### Why not drop the table

Also considered. It is the natural conclusion of "the directory is the row of
record", and it is one migration away. It is not taken here because the admin
install flow writes to it and works, `installed_by` and `created_at` are the
only record that an operator installed something and when, and deleting an
audit trail to tidy up a decision is a poor trade. The table is narrowed
rather than removed, and the narrowing is written down so the next person to
find it empty knows why it is empty rather than assuming a seeding bug.

## Consequences

- Adding a bundled system with no server crate is now **one directory**. No
  row, no registration, no restart-with-a-flag. Proved end-to-end: a pack
  created under `packs/systems/` with nothing else changed was listed by the
  running product, in title order, and disappeared when the directory did.
- A second hand-kept list surfaced while proving it, and is fixed here.
  `data/packs/systems/` — the directory the server actually reads — was a farm
  of symlinks made by hand on one afternoon in August, kept in step with
  nothing. A pack added to the repository was silently not offered by
  `node scripts/dev.mjs`: no error, no warning, just a picker that did not list
  it. `scripts/e2e-parallel.mjs` had always derived those links from the
  directory on every run; `scripts/dev.mjs` now does the same, and prunes links
  whose pack is gone.
- `check-system-registry.mjs` continues to fail the build if a system
  identifier appears in shared server code. The rule this ADR settles is the
  same rule, applied to where the *list* comes from rather than to where a
  branch is.
- **`system.json` is now load-bearing for discovery.** A pack whose manifest is
  malformed does not appear at all, rather than appearing and failing later.
  That is the intended failure mode — it fails in the picker, where a Game
  Master can see it — but it means a manifest typo removes a system silently
  from the list rather than loudly from a page.

## What would change this

- **A system can be installed at runtime and there are rows.** If the admin
  install flow becomes a path people actually use, the table stops being an
  audit trail and starts being a catalogue, and the two sources need
  reconciling rather than ranking. ADR-029 is the gate on that, not this ADR.
- **Systems need per-instance state the directory cannot hold** — an enabled
  flag, a pinned version, a per-realm restriction. A directory has no room for
  a fact about *this deployment's relationship* to a pack, and that is exactly
  what a row is for.
