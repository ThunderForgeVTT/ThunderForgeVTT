# Data Model: A Fight That Resolves

**Spec**: [spec.md](./spec.md) | **Research**: [research.md](./research.md)

Postgres through Diesel, one migration directory per change with paired
`up.sql`/`down.sql` (constitution). Every new table carries `created_by` and
`updated_by` (Principle III). Phase numbers refer to [plan.md](./plan.md).

## Changed: `tokens` (phase 3)

| Column | Type | Rule |
|---|---|---|
| `linked` | `BOOLEAN NOT NULL` | Backfilled `true` where `token_type = 'character'` and `actor_id` is set, `false` otherwise. New tokens: resolved by `createToken` (below). |
| `system_data` | `JSONB NULL` | Present only when `linked = false`. The copy's own `resource_data`-shaped object, validated by the pack's validator on every write. |
| `health`, `max_health` | *dropped* | Before dropping, a non-null `health` on an unlinked token is copied into `system_data` through the pack's declared hit-point fields. |
| `actor_id` | gains `REFERENCES world_actors(id) ON DELETE SET NULL` | Existing dangling ids are nulled first. |

**Constraint**: `CHECK (linked = false OR system_data IS NULL)`.

**`createToken` placement defaults (FR-016)**:
- no actor → `linked = false`, `system_data = NULL` (a marker; no bars)
- actor with `actor_type = 'character'` → `linked = true`
- NPC actor with `is_unique = true` → `linked = true`
- any other NPC actor → `linked = false`, `system_data` ← a copy of the
  actor's `resource_data` at that moment
- `token_type`, when the caller gives none, ← the actor's `actor_type`
  (`npc` for NPCs; fixes NPC tokens typed `character`)
- a caller may pass `linked` explicitly; a Game Master may change it later
  with `setTokenLink`

**Relinking** (`setTokenLink(tokenId, linked)`): unlinked → linked discards
`system_data` (spec edge case: not merged); linked → unlinked seeds
`system_data` from the actor.

## Changed: `world_actors` (phase 3)

| Column | Type | Rule |
|---|---|---|
| `is_unique` | `BOOLEAN NOT NULL DEFAULT false` | Settable by a Game Master on NPCs; ignored for characters, which are always linked by default. |

## Changed: `worlds` (phase 4)

| Column | Type | Rule |
|---|---|---|
| `auto_apply_npc_damage` | `BOOLEAN NOT NULL DEFAULT false` | GM-only setter. Hand-kept defaults in `mutations_worlds.rs` and `adapters.rs` updated too. |

## Changed: `world_combats` (phases 4, 7)

| Column | Type | Rule |
|---|---|---|
| `auto_apply` | `BOOLEAN NULL` | `NULL` = the world's setting. Ends with the combat (FR-006). |

## Changed: `world_combatants` (phases 1, 7)

| Column | Type | Rule |
|---|---|---|
| `downed_by` | `TEXT NULL CHECK IN ('hit_points','game_master')` | Set with `active = false`. Healing reactivates only `'hit_points'`. The GM's Down button writes `'game_master'`; Up clears it. |
| `kind` | `TEXT NOT NULL DEFAULT 'creature' CHECK IN ('creature','lair')` | A lair has no token or actor, initiative 20, tiebreak −1. |

## New: `world_combatant_budgets` (phases 6, 7)

One row per combatant, created with the combatant.

| Column | Type | Rule |
|---|---|---|
| `combatant_id` | `UUID PK REFERENCES world_combatants ON DELETE CASCADE` | |
| `action_spent` | `INT NOT NULL DEFAULT 0` | Never capped; 2 of 1 is a visible overspend. |
| `bonus_action_spent` | `INT NOT NULL DEFAULT 0` | |
| `reaction_spent` | `INT NOT NULL DEFAULT 0` | |
| `movement_spent` | `DOUBLE PRECISION NOT NULL DEFAULT 0` | System units (feet for 5e). |
| `legendary_per_round` | `INT NULL` | From the pack's declared field when the combatant was added. |
| `legendary_remaining` | `INT NULL` | Refilled at the start of the owner's turn; may go negative. |
| `created_by`, `updated_by`, `created_at`, `updated_at` | | |

**State transitions** (on `advanceTurn` to combatant C): C's action, bonus
action, movement and reaction → 0 spent; C's `legendary_remaining` →
`legendary_per_round`. Nobody else's budget changes.

*Implemented (tasks T085, T087):* migration
`2026-09-14-230000-0000_combatant_budgets` backfills a row for every existing
combatant (attributed to its combat's `created_by`). A row is created by
`addCombatant`; a combatant whose row is missing reads as nothing spent and
gets one on its first spend or turn. The same reset runs when removing the
active combatant hands the turn on. What is allowed is not stored (research
R13). The legendary columns exist and stay null until Phase 9.

## New: `world_attacks` (phase 4)

The record of one attack, and the source of every seat's view of it. One
row per attack in a multiattack.

| Column | Type | Rule |
|---|---|---|
| `id` | `UUID PK` | |
| `world_id`, `scene_id`, `combat_id` | `UUID` (`combat_id` nullable) | |
| `attacker_token_id` | `UUID NULL` | Null only for a lair action. |
| `target_token_id` | `UUID NULL` | Null = "a roll into the air": nothing is offered or applied. |
| `ability_id` / `item_id` | `UUID NULL` (at most one set) | `ON DELETE SET NULL`, so "exactly one" holds at creation and "at most one" after a deletion. |
| `attacker_label`, `target_label` | `TEXT` (`target_label` nullable) | *Added in implementation.* The names when the attack was made, so a deleted token still reads in the Game Master's log. Server-side only: sent to a viewer only when that party is not redacted. |
| `ability_name` | `TEXT NOT NULL` | *Added in implementation.* The ability's or item's name when made; withheld when the attacker is redacted. |
| `multiattack_of` | `UUID NULL REFERENCES world_attacks` | The parent attack for a multiattack's parts: the first part. |
| `to_hit_roll_id`, `damage_roll_id` | `UUID NULL REFERENCES world_roll_records` | Damage roll only on a hit. |
| `defence` | `INT NULL` | The value the total was compared with; null when the target has none. |
| `outcome` | `TEXT NOT NULL CHECK IN ('hit','miss','no_defence','no_target')` | |
| `distance` | `DOUBLE PRECISION NULL` | System units, from footprint to footprint. |
| `flags` | `TEXT[] NOT NULL DEFAULT '{}'` | Any of `out_of_reach`, `long_range`, `beyond_range`, `no_line_of_sight`, `no_reach_declared`, `overspent`, `legendary_on_own_turn`. |
| `action_cost` | `TEXT NOT NULL` | As spent. |
| `created_by`, `updated_by`, `created_at`, `updated_at` | | |

`world_roll_records` is unchanged: attack rolls are ordinary roll records,
linked from here.

## New: `world_offers` (phase 4)

| Column | Type | Rule |
|---|---|---|
| `id` | `UUID PK` | |
| `world_id`, `scene_id` | `UUID` | |
| `attack_id` | `UUID NULL REFERENCES world_attacks` | Null for a Game Master's direct offer of healing. |
| `target_token_id` | `UUID NOT NULL REFERENCES tokens ON DELETE CASCADE` | Controllers are resolved at read and resolve time (research R7), not stored. |
| `target_linked` | `BOOLEAN NOT NULL` | *Added in implementation.* The token's link state when the offer was made; taking the offer after a relink is refused (research R18, C6a). |
| `kind` | `TEXT NOT NULL CHECK IN ('damage','healing')` | |
| `amount` | `INT NOT NULL CHECK (amount >= 0)` | |
| `status` | `TEXT NOT NULL DEFAULT 'pending' CHECK IN ('pending','taken','declined','applied')` | `applied` = auto-apply, never pending. |
| `resolved_by` | `UUID NULL REFERENCES users` | |
| `resolved_on_behalf` | `BOOLEAN NOT NULL DEFAULT false` | True when a Game Master resolved a player's offer (FR-009). |
| `resolved_at` | `TIMESTAMPTZ NULL` | |
| `created_by`, `updated_by`, `created_at`, `updated_at` | | |

**State transitions**: `pending → taken | declined`, once, by a controller or
a Game Master; `applied` is terminal at creation. A pending offer never
expires (FR-008). Taking an offer calls the hit-point operation; the offer
row and the hit-point write commit together.

**Index**: `(target_token_id) WHERE status = 'pending'`, for "your pending
offers" on reconnect.

## Pack manifest additions (phases 1, 4, 5, 6, 7)

A new top-level `combat` block, typed in `pack_system_spec` and documented in
`packs/systems/README.md`:

```jsonc
"combat": {
  "hitPoints": { "slot": "resourceData", "current": "current_hp",
                 "max": "max_hp", "temporary": "temporary_hp" },   // phase 1
  "defence":   { "slot": "abilityData", "field": "armor_class",
                 "label": "Armour Class", "abbrev": "AC" },          // phase 4
  "sizes": {                                                          // phase 5
    "source": { "slot": "traitData", "field": "size" },
    "categories": [ { "id": "medium", "label": "Medium", "footprint": 1 }, … ]
  },
  "legendary": { "slot": "traitData", "field": "legendary_actions" } // phase 7
}
```

and `turnStructure.budget` (phase 6):

```jsonc
"turnStructure": { "rounds": true, "roundLabel": "Round",
  "budget": { "action": 1, "bonusAction": 1, "reaction": 1,
              "movement": { "speed": "walk" } } }
```

The 5e pack adds `armor_class` and `size` and `legendary_actions` to its data
types. Genie's `sizeCategories` moves under `combat.sizes` and its validator
reads the manifest.

## Ability and item additions (phases 4, 5, 6, 7)

On `world_abilities` and `world_items` (the ability-or-item is the attack,
research R2):

| Column | Type | Rule |
|---|---|---|
| `reach` | `DOUBLE PRECISION NULL` | System units. |
| `range_normal`, `range_long` | `DOUBLE PRECISION NULL` | `range_long >= range_normal` when both set. |
| `needs_line_of_sight` | `BOOLEAN NOT NULL DEFAULT true` | Settable by import (FR-007). |
| `action_cost` | `TEXT NOT NULL DEFAULT 'action' CHECK IN ('action','bonus_action','reaction','legendary','free')` | |
| `legendary_cost` | `INT NOT NULL DEFAULT 1` | |
| `multiattack` | `UUID[] NOT NULL DEFAULT '{}'` | Ids of abilities in the same world. |

Carried through collection copies (spec 026) and exports like every other
ability field.

## Derived (not stored)

- **Footprint** of a token: its actor's size (linked) or its NPC's size
  (copy), through `combat.sizes`; 1 when unknown. Served by `tokenGrid`.
- **Controllers** of a token: `owner_user_id` ∪ Owner-permission holders on
  its actor; empty → the world's Game Masters.
- **Visibility of a token to a viewer**: any of the viewer's controlled tokens
  in the scene sees it under `vision::visibility_of`; Game Masters see all.
