# Contract: `packs/systems/roll_for_shoes/server/`

A small crate with one job: make the system known to the registry, and give
its two stored slots a shape.

## Why it exists at all

The pack contract calls `server/` optional, and it is — for a pack that only
declares. Roll for Shoes stores XP and a skill lineage, and
`update_actor_system_data` validates every write through
`GameSystemRegistry::validate`, which refuses a system the registry does not
hold. The registry is populated only from `inventory`-submitted
`SystemContribution`s. Without this crate the sheet cannot save anything.
Research D2 records the evidence.

## What it submits

```rust
inventory::submit! {
    thunderforge_canvas_core::system_contribution::SystemContribution {
        trait_data: Some(validate_trait_data_for_registry),
        resource_data: Some(validate_resource_data_for_registry),
        ..SystemContribution::new(SYSTEM_ID)
    }
}
```

`SYSTEM_ID` must equal the manifest's `id`. Every other field stays absent:

| Field | Why absent |
|---|---|
| `ability_data` | the game has no attributes |
| `proficiency_data` | it has no proficiencies |
| `spell_data` | it has no magic system |
| `rules` | it derives nothing — a skill's pool size *is* its level, and XP is a stored count |

## The validators

```rust
type ValidatorFn = fn(&serde_json::Value) -> Result<(), String>;
```

Written against a richer internal error and adapted at the boundary, the way
Blades in the Dark does it. The rules they enforce, and the message each
produces, are the tables in [`../data-model.md`](../data-model.md#validation).

Two things they deliberately do not do:

- **They do not judge a skill's name.** Specificity and relevance are the
  table's call (FR-036). Any non-empty name passes.
- **They do not cap a level.** There is no upper bound (FR-040).

## What it does not do

- **No root GraphQL fields.** The pack merges no query, mutation or
  subscription type, so it submits no `PackSurface` and has nothing to classify
  for a paused world. It rides `rollDice` and `updateActorSystemData`, which
  already refuse one.
- **No migration, no table, no column.**
- **No `engine/` crate.** Roll for Shoes has no geometry.

## The lines outside the pack directory

Exactly the ones the pack contract allows, and no more:

| File | Line |
|---|---|
| `Cargo.toml` | the crate's `members` entry |
| `src/app/Cargo.toml` | the dependency |
| `src/app/src/system_packs.rs` | `use roll_for_shoes_server as _;` |

Each is a build-graph fact — a crate exists and should be linked — and says
nothing about what the crate contains, so none of them can drift out of step
with the pack. `scripts/check-system-registry.mjs` fails the build if the
system's id appears anywhere else in shared server code, which is the check
SC-008 is measured by.
