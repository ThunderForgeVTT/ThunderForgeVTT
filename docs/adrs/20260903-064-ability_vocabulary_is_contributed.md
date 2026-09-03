# Ability Vocabulary Is Contributed, and the CHECK Constraint Goes

- **Date**: 2026-09-03
- **Status**: Accepted
- **Spec**: `specs/033-abilities-vocabulary/` (FR-011 to FR-017, SC-003)
- **Related**: ADR-054 (the contribution seam), ADR-061 (discovery, not a
  registry), ADR-028 (the directory is the row of record)

## Context

An ability carries a type — `spell`, `feat`, `power`, `talent`. Spec 033 asks
for two things about it: that a game system supply its own words for those
types and its own umbrella term for the concept, and that a system be able to
name types the application has never heard of. 5e has Enchantments; Genie has
Scrolls and Knacks; Blades has neither word.

Three closed lists stand in the way, and the first is the one that matters:

```sql
classification VARCHAR(16) NOT NULL
    CHECK (classification IN ('spell', 'feat', 'power', 'talent'))
```

`src/server/migrations/2026-08-25-120000-0000_create_world_abilities/up.sql:19-20`.
The other two are `AbilityClassification` as a GraphQL enum
(`src/server/src/graphql/types.rs:1046`) and as a TypeScript union
(`apps/web/src/types/ability.ts:14`).

FR-012 and SC-003 require that adding a type for one game system change
**zero** files shared with other systems, verifiable by an automated check. A
constraint enumerating the valid values makes every new type a migration, and a
migration is as shared as a file gets.

There is no arrangement in which both survive.

## Decision

**A game system contributes its ability vocabulary, and the database stops
enumerating valid types.**

1. **The vocabulary is declared in the manifest**, not in a pack crate. A
   system declares an umbrella term and a list of types, each with a label, a
   plural, an order, and its binding and grading facets. Every part is
   optional. This extends `abilityFacets`, which
   `packs/systems/genie/system.json` already ships, rather than inventing a
   second mechanism.
2. **The CHECK constraint is dropped.** Validity becomes "the world's assembled
   vocabulary contains this identity", enforced server-side in the ability
   mutations — where every other authoring rule in this codebase is enforced,
   per Constitution Principle III.
3. **The GraphQL enum and the TypeScript union are retired.** The wire type is
   a string identifier, described by the vocabulary the same request can fetch.
4. **The four built-ins remain permanently authorable** (FR-017). A declaration
   matching a built-in id re-labels it and never creates a second type.
5. **Presence is not availability.** A built-in is *shown* when the active
   system declares or re-labels it, or when the world holds at least one
   ability of that type (FR-011a). A 5e world does not carry empty "Powers" and
   "Talents" tabs it can never clear, and no ability can be hidden by the rule,
   because holding one is itself sufficient to show the tab.
6. **The property is enforced by a check, not by discipline.**
   `scripts/check-ability-vocabulary.mjs` collects every type identity any pack
   declares, subtracts the built-ins, and fails if one appears as a literal in
   shared code — deriving the ids from `packs/systems/` itself, the way
   `check-system-registry.mjs` does, so it cannot go stale.

### Why a constraint could not have worked anyway

Even setting FR-012 aside, a table-wide constraint cannot express the rule that
actually applies. FR-013 says a type declared by one system must not be offered
in a world running a different one. That is a **per-world** question, and no
column constraint can see the world's system. The constraint was not merely
inconvenient; it was answering a different question from the one being asked.

## Consequences

- **A bad `classification` can now reach the database only through a bug in the
  mutation layer**, rather than being refused by Postgres. That is the same
  trust already placed in every other authoring rule here — `gm_only`,
  permission levels, world membership — none of which has a constraint either.
  Stated plainly because it is a real reduction in one kind of safety, bought
  for a kind the constraint could not provide.
- **A lossy fallback becomes a live bug and is fixed with this.**
  `types.rs:1210` and `mutations_ability_shares.rs:114` read an unknown stored
  value as `unwrap_or(AbilityClassification::Spell)` — an ability of an
  unrecognised type silently presented as a Spell, which FR-034 forbids in as
  many words. Nothing can write a fifth value today, which is the only reason
  nobody has hit it.
- **The vocabulary is assembled per world**, not cached per system, because
  FR-011a's presence rule depends on what the world holds.
- **Six web components stop reading the manifest themselves.**
  `WorldCompendiumPage`, `AbilityCompendiumTab`, `AbilityPreviewPanel`,
  `AbilityDetailPage`, `ActorAbilitiesPanel` and the shared-ability page each
  cast `abilityFacets` today. FR-006 requires all six to agree, and six readers
  is six chances not to.
- **The down migration cannot restore the constraint unconditionally.** By then
  rows may legitimately hold a system-declared type. It restores it only if
  every row still holds a built-in, and otherwise fails loudly rather than
  deleting somebody's abilities to make a constraint fit.

## Alternatives considered

- **Regenerate the constraint per installed pack.** Rejected: it makes the
  database schema a function of which packs are installed, so installing a pack
  becomes a migration and uninstalling one makes existing rows invalid.
- **A `world_ability_types` table seeded from manifests.** Rejected for the
  reason ADR-028 gives for `game_systems`: it makes the database a cache of the
  filesystem with the filesystem still authoritative, and adds a staleness mode
  for no gain.
- **Contribute the vocabulary from the pack's Rust crate** via
  `SystemContribution`, now that packs can. Rejected — it would make a purely
  declarative naming table require a compiled crate, excluding manifest-only
  packs from a feature that is entirely about names. ADR-029 says an outside
  pack is data; a pack with no crate must be able to name its own types.

## What would change this

- **A type needing bespoke behaviour**, not just a name and a shape. The line
  ADR-054 drew holds here: a manifest may not declare a capability no code can
  perform. A type declares *that* it binds to items and *that* it is graded;
  the application performs both generically. The moment a type needs its own
  logic, this is the wrong home for it.
- **A performance problem from assembling per world.** It is a manifest read
  and a set operation on a page load today. If that ever matters, the answer is
  a cache keyed by (system, classifications-in-use), not a table.
