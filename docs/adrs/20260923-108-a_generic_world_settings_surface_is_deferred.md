# ADR-108: A Generic World-Settings Surface Is Deferred, Not Rejected

**Date:** 2026-09-23
**Status:** **ACCEPTED** 2026-09-23. Proven by spec 062's Phase 2 — the pack-owned
table `world_roll_for_shoes_settings` with its migration, its `except_tables` entries
in both `diesel.toml` files, its two contributed root fields and the `PackSurface`
classification that `play_pause_surface_tests` genuinely exercises (T012), with
`scripts/check-system-registry.mjs` green on an empty `KNOWN` list.
**Participants:** ThunderForgeVTT Team
**Related:** [ADR-063](./20260903-063-a_pack_owns_the_tables_it_writes.md) (the governing
storage decision and the threshold quoted below), [ADR-091](./20260907-091-instance_configuration_is_rows.md)
(the instance-scoped store this would mirror), spec 062 (research D1, FR-001–FR-008),
constitution Principle III (data has one owner) and Principle IV

---

## Problem Statement

Roll for Shoes' five Extras are per-world settings. A setting that lives
nowhere cannot vary by world, so the feature needed somewhere to put them —
and the product has no per-world, per-system configuration surface at all.

ADR-063 already decides *where a pack's data goes*: the pack owns the table it
writes. Applying it here is not a new decision and needs no ADR.

What does need one is the threshold ADR-063 sets for itself:

> One pack wanting something is a case; two is a shape.

Genie already stores a per-world setting. Roll for Shoes now wants five. That
is the second case, so by ADR-063's own test the shape has appeared and a
generic surface is *justified* — at exactly the moment a feature was shipped
without building one. Whoever meets the third case deserves to find out why,
rather than re-deriving it or assuming the question was never asked.

## Decision

**Build the pack-owned table now; do not build the generic surface as part of
this feature. Record the threshold as met so the third case starts from a
decision instead of a blank page.**

1. **The five settings live in `world_roll_for_shoes_settings`**, one row per
   world, keyed on `world_id` with `ON DELETE CASCADE`, declared and written
   only by `roll-for-shoes-server`. Five named, typed columns — not a JSON
   blob. This is ADR-063 applied unchanged.

2. **No column goes on `worlds`.** This is the one option actively *rejected*
   rather than deferred, and it is rejected permanently. It is how Genie's
   `worlds.genie_resource_carryover_enabled` works, and Genie's own panel
   source files that as a known defect: a column named for one ruleset on the
   table every system shares. `check-system-registry.mjs` would not catch it —
   a column name is neither a quoted system id nor a filename — and passing the
   check that exists to enforce a rule while breaking the rule is worse than
   failing it. Five such columns would turn one mistake into five.

3. **A world with no row reads as every default, and every default is off.**
   Nothing is seeded. The row is written on first change. This is what makes
   "a world that touches none of these plays exactly the core game" a property
   of the storage rather than a promise the UI keeps.

4. **The generic surface, when it is built, is its own spec.** It is not a
   generic key/value store — ADR-063 rejected that shortcut explicitly, as a
   different design for a different problem. ADR-091 shows the real shape at
   instance scope: rows keyed by a string that a **registry declares**, plus an
   append-only table of the changes. The declaration is what separates it from
   a key/value bucket, and a world-scoped counterpart needs the same — declared
   keys, declared types and defaults, a rule for who may write each one, and
   the audit trail beside it. That is a specification, not a paragraph of a
   ruleset's feature.

## Consequences

**`roll-for-shoes-server` stops being a validator-only crate.** It gains
`diesel`, `async-graphql` and `thunderforge-server`, a migration under
`src/server/migrations/`, an `except_tables` entry in both `diesel.toml` files,
and a `PackSurface` declaration classifying its read as `reads` and its write
as `gated` with a request document. Genie's
`packs/systems/genie/server/src/session/` is the worked example for every one
of those steps. This is the honest price of the feature: four of the five
Extras are pure `game.ts` changes, and it is the settings alone that need a
server.

**Three shared paths are touched, and each is unavoidable.** The migration
directory is one directory by Diesel's design; `src/app/src/schema_roots.rs`
names the type at the composition root, which is what contributing a root
field *means*; and `src/server/Cargo.toml` gains a dev-dependency so the
server's test binary links the pack and the play/pause surface test sees the
new `PackSurface` instead of passing vacuously. None of them names the system
inside shared *source*, so the registry check stays green with `KNOWN` empty.

**The choice is forward-compatible, which is the reason it is affordable.** A
pack-owned table of five named, typed columns migrates into a declared registry
by moving rows. A column on `worlds` would need a migration *and* a rename
*and* a deprecation — the loose end Genie is carrying, and the reason that
option is rejected outright rather than deferred alongside this one.

**The panel and the sheet each read the settings themselves.** The
`world-settings` slot hands a panel `world: WorldRecord`, which does not carry
these settings and will not, because they are not on `worlds`. Both
`panels/world-settings.tsx` and `ActorSheet.tsx` query for them through
`postGraphQL` from `@thunderforge/host` — the same transport every other caller
uses — exactly as Genie's panel posts its own GraphQL.

**The debt is named, not hidden.** Two packs now carry per-world settings in
two different shapes, one of them a known defect. That is the cost of
deferring, and it is the cost this ADR exists to make visible.

## What the third case should do

Read this ADR first, and then do **not** add a third bespoke shape.

The threshold is already met; a third case makes it emphatic. The work is a
world-scoped counterpart to ADR-091's `instance_settings`: a pack declares its
settings — key, type, default, who may write it — and the host stores,
validates and audits them, with the pack keeping no table of its own.
Its first migration is the interesting part, and it has two known inputs:
`worlds.genie_resource_carryover_enabled`, which must also lose its column,
and `world_roll_for_shoes_settings`, whose five typed columns are already the
right shape to lift across.

If the third case is small enough that a table of its own is genuinely cheaper
than the registry, say so in that spec and cite this ADR — but say it, rather
than reaching for the pack-owned table because it is the path already worn.
