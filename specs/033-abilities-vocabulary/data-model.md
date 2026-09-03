# Data Model: An Open Ability Vocabulary and a Guarded System Switch

**Spec**: `specs/033-abilities-vocabulary/spec.md` | **Date**: 2026-09-03

Most of this feature adds no storage. The vocabulary is read from manifests and
assembled per request; unrecognised-ness is a presentation state computed from
the active system, never a stored flag (FR-034, and the spec's own decision).

What follows is the whole of the persistent change.

## Schema changes

### 1. `world_abilities.classification` — the CHECK constraint is dropped

```sql
ALTER TABLE world_abilities DROP CONSTRAINT world_abilities_classification_check;
```

The column stays `VARCHAR(16) NOT NULL`. What may be stored in it becomes a
function of the world's active system's vocabulary, enforced in the ability
mutations — see `contracts/ability-vocabulary.md`.

**No data migration.** Every existing row holds one of the four built-ins,
which remain permanently built in (FR-017). Nothing is re-typed, and no world
requires GM action (SC-012).

**Reversibility**: the down migration cannot restore the constraint
unconditionally, because by then rows may legitimately hold a system-declared
type. It restores it only if every row still holds a built-in, and otherwise
fails loudly rather than deleting anybody's abilities to make a constraint fit.

### 2. `world_abilities.grade` — new, nullable

```sql
ALTER TABLE world_abilities ADD COLUMN grade INTEGER;
```

The value on a type's declared grade — 5e's spell Level, another system's Rank
or Circle. `NULL` means the ability's type declares no grade, which is the
common case and must stay the cheap one.

**Out-of-range values are retained, never clamped** (FR-023). A system that
narrows a range later does not get to edit content authored under the old one,
so this column is *not* constrained to any range; the range is checked at
authoring time against the vocabulary in force.

### 3. `world_item_abilities` — new table

The item counterpart of `world_actor_abilities`, mirroring it deliberately.

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID PK` | |
| `item_id` | `UUID NOT NULL` | `REFERENCES world_items(id) ON DELETE CASCADE` |
| `ability_id` | `UUID NULL` | `REFERENCES world_abilities(id) ON DELETE SET NULL` |
| `ability_name_snapshot` | `TEXT NOT NULL` | survives the ability's deletion |
| `created_by` / `updated_by` | `UUID NOT NULL` | `REFERENCES users(id)` — Principle III |
| `created_at` / `updated_at` | `TIMESTAMP NOT NULL` | |

**Why nullable `ability_id` with a snapshot**: exactly the existing pattern in
`world_actor_abilities` (`schema.rs:489-497`), where a deleted ability leaves a
tombstone rather than vanishing from the sheet it was on. Diverging here would
mean two behaviours for one concept.

**Why a table and not a column on `world_abilities`**: an ability may be
attached to many items, and the attachment carries its own provenance. A column
would also make "attached" a property of the ability rather than of the
relationship, which the character side already decided against.

**Not merged with `world_item_effects`.** An effect is a mechanical rule the
resolution layer consumes; an ability is named, described, permissioned,
shareable content. They are reconciled on the item, in presentation, and
nowhere else (FR-020, and the spec's decisions).

## Entities that are computed, not stored

### Ability Vocabulary

Assembled per world from the built-ins plus the active system's manifest
declaration. Never persisted — the manifest is the record, and storing a copy
would make the database a cache of the filesystem, which ADR-028 rejected for
`game_systems` for the same reason.

```text
AbilityVocabulary
├── umbrella: { label, plural_label }        # defaults to Ability/Abilities
└── types: [ AbilityTypeDeclaration ]        # in declared order, built-ins first
```

```text
AbilityTypeDeclaration
├── id: String                    # stable identity; matches the stored column
├── label / plural_label: String  # never empty; falls back to the id
├── order: i32
├── builtin: bool                 # true for the four; a GM cannot tell
├── binds: Character | Item | Nothing   # exactly one, never a set
└── grade: Option<{ label, min, max }>
```

**Assembly rules**, all total — no input produces an error or a blank label:

1. Start from the four built-ins: `spell`, `feat`, `power`, `talent`.
2. A declaration whose `id` matches a built-in **re-labels it** — one tab, not
   two (FR-014).
3. A declaration with a new `id` adds a type.
4. A malformed declaration is skipped; the rest of that system's vocabulary
   survives (FR-016).
5. A missing label falls back to the id, never to blank (FR-016) — the
   behaviour `abilityFacets.ts:78-110` already implements.
6. Two declarations irreconcilably claiming one identity are reported **at
   assembly** (FR-015), not when a GM first authors one.
7. A built-in is **present** when the system declares or re-labels it, or when
   the world holds at least one ability of that type (FR-011a). Presence is
   therefore a function of the world as well as the system — which is why the
   vocabulary is assembled per world and not cached per system.

### Unrecognised-Type Ability

A stored ability whose `classification` is not in the assembled vocabulary. A
**presentation state**: nothing about the row changes when it enters or leaves
this state, and switching the system back restores it exactly (FR-036, SC-008).

Presented as a **final tab** in the same tab set, only while such abilities
exist, offering no creation, and labelled with the stored identity itself
(FR-035, FR-035a).

This is the state that today's `unwrap_or(AbilityClassification::Spell)` at
`types.rs:1210` silently erases, and it is why that fallback goes.

### Content Inventory

Counted per world when a system-change confirmation opens; never stored.

**Actors, abilities and items only.** Scenes and lore are excluded: every world
ships with a default scene (spec 010), so counting scenes would make every
world non-empty and FR-029's one-click path unreachable.

```text
ContentInventory
├── counts: [ { kind, system_id, count } ]   # actors, abilities, items — and nothing else
├── becoming_unrecognised: u32               # requires the target vocabulary (FR-037)
└── digest: String                           # over the above; see the guard contract
```

## What is deliberately unchanged

- `world_abilities` keeps its identity, ownership, effects, permissions and
  share links. A share link to an ability of a now-unrecognised type keeps
  working.
- `world_actor_abilities` is untouched. The item table is a peer, not a
  replacement.
- `world_item_effects` is untouched.
- `worlds.game_system_id` is untouched — the guard constrains *how* it changes,
  not what it stores.
