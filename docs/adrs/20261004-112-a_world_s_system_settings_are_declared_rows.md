# ADR-112: A World's System Settings Are Declared Rows

**Date:** 2026-10-04
**Status:** **ACCEPTED** 2026-10-04. Proven by spec 067 Story 1 — the `settings` block in `crates/pack_system_spec`, the shared tables `world_system_settings` and `world_system_setting_changes`, the `worldSystemSettings` and `setWorldSystemSetting` root fields, and 5e's `inspiration` setting, which reached a player's open sheet in `apps/web/e2e/system-world-settings.spec.ts` with no migration, GraphQL type or panel written for it.
**Participants:** ThunderForgeVTT Team
**Related:** [ADR-108](./20260923-108-a_generic_world_settings_surface_is_deferred.md) (the deferral this closes), [ADR-063](./20260903-063-a_pack_owns_the_tables_it_writes.md) (a pack owns the tables it writes), [ADR-091](./20260907-091-instance_configuration_is_rows.md) (instance configuration is rows), spec 067 (FR-001 to FR-013)

---

## Problem Statement

A game system leaves choices to the table: a house rule on or off, how many of
something, which of a few modes. ADR-108 recorded that the product had no
per-world, per-system place to keep those choices, that Genie and Roll for
Shoes had each built their own table, and that a generic surface was deferred
until a third case showed the shape.

The pack audit of 2026-10-04 was that third case. Every one of the seven
remaining systems has table choices of this kind, and under ADR-108 each would
cost a migration, two `diesel.toml` entries, a GraphQL type, two root fields, a
pause-surface classification and a panel. That price is why none of them had
any.

## Decision

1. **The pack declares; the host stores.** A system manifest's `settings`
   block names each choice: `id`, `label`, `type` (`boolean`, `integer`,
   `choice` or `text`) and `default`, with bounds or options where the type has
   them. `validate_system_manifest` refuses a block a form could not be built
   from.

2. **One shared table.** `world_system_settings` holds a world's answers,
   keyed by world, system and key, with a JSON value. A pack writes no
   migration to add a setting.

3. **A value that is not both declared and allowed does not exist.** A world
   with no row reads as the declared default. So does a row for a key the
   manifest no longer declares, and a stored value the declaration has stopped
   allowing. None of those rows is deleted, so a setting that is dropped and
   restored finds the world's answer where it was left. Nothing is coerced.

4. **Every change is recorded.** A write appends to
   `world_system_setting_changes` in the same transaction: who, when, the old
   value and the new. This is ADR-108's decision 4, kept.

5. **Any member reads; the Game Master writes.** One mutation sets one
   setting. A paused world refuses it. Each write announces world event 35
   carrying the key, and clients read again.

6. **The host draws the form.** The world's System settings page renders a
   control per declared setting from the query's answer, above the pack's
   `world-settings` panel. A pack reads a value back through
   `useWorldSystemSettings` from `@thunderforge/host`, or on the server through
   `world_system_settings::effective_value`.

7. **A pack may add a check.** `SystemContribution::world_setting` runs after
   the declared check, for a rule the four types cannot express.

8. **Genie and Roll for Shoes keep their tables.** They work, they are tested,
   and Roll for Shoes' difficulty is not a setting in this sense. Moving them
   is a separate decision nobody has asked for.

## Rationale

ADR-063 rejected a generic key/value store because an undeclared bag has no
owner and no validation. This is not that store: every key is declared by the
pack that reads it, every value is checked against that declaration on write
and again on read, and the shared table is addressed only through one module.
The pack still owns the meaning; the host owns only the storage, which is the
part every pack would otherwise write identically.

Reading invalid rows as the default, rather than failing or deleting, follows
ADR-091's registry at instance scope. A manifest changes between releases, and
a world must still open.

## Consequences

- A new setting is a manifest entry and whatever code reads it.
- Changing a `default` changes every world that never chose. The pack contract
  says so.
- The four types are the whole vocabulary. A setting that needs more than they
  express still takes ADR-063's route: the pack's own table and its own panel.
- The form cannot group, hide or relate settings to each other. If a system
  needs that, it is a new decision.
- ADR-108's deferral is closed by this record.

## Alternatives Considered

**A table per pack, as before.** Correct and already working twice, but its
cost is the reason seven systems have no settings.

**A JSON column on `worlds`.** No migration per pack either, but one value for
all systems means a world that changes system carries or loses the previous
system's answers, a concurrent write of two settings loses one, and there is no
row to record a change against.

**A settings type per pack contributed through GraphQL.** Typed end to end,
but it keeps the per-pack GraphQL, pause classification and panel that the
shared surface exists to remove.
