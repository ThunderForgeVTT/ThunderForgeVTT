# Phase 1 Data Model: Genie House System

Per research.md R2, Genie introduces **no new database tables for its own content** — every entity below maps onto an existing, generic table from a prior spec. This file documents that mapping plus the one real schema change in scope (widening `scenes.grid_type`).

## Genie Character (manifest-level shape, stored via existing actor `data_types`)

Structurally identical in storage terms to how `dnd5e` characters already work — no new table, no new column. The manifest (`packs/systems/genie/system.json`) declares `data_types` blocks the same way `dnd5e/system.json` does, and the existing actor system data path validates against them.

| Field group | Shape | Notes |
|---|---|---|
| `ability_data` | object, one integer property per Genie ability | Mirrors `dnd5e`'s `ability_data` block structurally (research.md R2) |
| `resource_data` | object | Includes `current_wish_points` / `max_wish_points`, mirroring `dnd5e`'s `current_hp`/`max_hp` pattern |
| `proficiency_data` | object | Skill training flags, one boolean per Genie skill, mirroring `dnd5e`'s `skill_proficiencies` |
| `condition_data` | object | New shape (not present in `dnd5e`'s data_types, since 5e conditions aren't tracked this way in that digest) — an array of active condition names with optional duration, validated the same way other `data_types` blocks are |
| `patron_lore_entry_id` | UUID reference | Points at a `world_lore_entries` row (spec 012) — no FK enforced at the actor-data-JSON level (consistent with how system-specific actor data is stored today), validated at write time by the Genie server validator |

**Validation rules**: Same pattern as `packs/systems/dnd5e/server/src/validators.rs` — each `data_types` block's required properties are enforced server-side; `patron_lore_entry_id`, if present, must reference a lore entry within the same world (checked at write time, not via a hard FK, consistent with existing actor-system-data conventions).

## Genie NPC (manifest-level shape, same storage as Genie Character)

Distinct `data_types` block name (e.g. `npc_data_types` vs. `character_data_types`) so the server/web layers can tell a staged NPC apart from a player character, per FR-008 — same underlying storage mechanism as above, just a different declared shape in the manifest.

| Field group | Shape | Notes |
|---|---|---|
| `ability_data` | same as Character | |
| `resource_data` | same as Character, simplified (no distinction needed between current/max for a one-shot NPC, though both are kept for consistency) | |
| `size_category` | string enum: `diminutive`, `small`, `medium`, `large`, `huge`, `colossal` | New to Genie — resolves to a token `scale` value via the manifest's `sizeCategories` lookup table (research.md R6) |

## Manifestation Roll (no storage — a formula string resolved at request time)

Not a stored entity. Genie's manifest declares the formula (research.md R3) as a string, e.g.:

```json
"manifestationRoll": {
  "formula": "{skill}d6k{keep}!6cs>=4",
  "description": "Roll a pool of d6s equal to the relevant skill rating, keep the top N, explode on 6, count successes at 4+."
}
```

Resolved via the existing `rollDice` mutation (spec 014, contracts/graphql-roll.md), with `{skill}` and `{keep}` substituted as `PlaceholderBindings` at request time — no new roll-record shape, no new persistence; `world_roll_records` (spec 014) already stores the result generically.

## Wish Points Table (manifest-level, no storage)

```json
"wishPoints": {
  "1": [2], "2": [3], "3": [4], ...
}
```

Structurally identical to `dnd5e`'s `spellSlots` field (research.md R4) — a level-keyed lookup consumed the same way by derived-data recalculation on level-up. No new table; the character's *current* Wish Points total lives in `resource_data.current_wish_points` (above), recalculated from this table when level changes.

## Wish-Granted Item (stored as an ordinary `world_items` row + `world_item_effects` row)

No new columns. A Wish-Granted Item is simply a `world_items` row (spec 013) whose `world_item_effects.formula` is a dice-engine formula string (spec 013's existing item-effect shape, resolved by the same `crates/thunderforge-dice` engine as the Manifestation roll). Genie contributes no schema — only example item content shipped as pack data (not a database migration).

## Patron / Lineage (stored as an ordinary `world_lore_entries` row)

No new columns. A Patron is simply a `world_lore_entries` row (spec 012); the Genie Character's `patron_lore_entry_id` (above) is the only Genie-specific piece, and it's a plain UUID inside the actor's system-data JSON, not a new relational column.

## Scene Topology (the one real schema change)

| Table | Change |
|---|---|
| `scenes` | `grid_type` CHECK constraint widened from `IN ('square', 'hex')` to `IN ('square', 'hex', 'gridless')`, via a new migration (never editing the original `2026-05-05-010000-0001_create_scenes_table` migration in place, per existing Diesel convention: `up.sql` drops and re-adds the constraint, `down.sql` reverses it). |

No column changes — `GridType::Gridless` already exists engine-side (research.md R1); this migration only makes the value reachable from scene creation/update mutations.

## Condition (manifest-level list, referenced by `condition_data` above)

```json
"conditions": [
  { "key": "bound", "label": "Bound", "description": "..." },
  { "key": "exposed", "label": "Exposed", "description": "..." }
]
```

A static list on the manifest (like `skills`/`abilities`), not a database entity — a character's *active* conditions are just string keys inside `condition_data`.

## Session Wish Pool + Doom Clock (new table)

Per research.md R7, this is genuinely new state — no existing table models session-scoped shared party state.

### `world_genie_sessions`

One row per active Genie play session within a world (a world could in principle host more than one session over its lifetime; only one is "active" at a time, per FR-013/User Story 7).

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID` PK | |
| `world_id` | `UUID` FK → `worlds.id` | Scopes the session |
| `wishes_remaining` | `INT NOT NULL DEFAULT 3` | The Session Wish Pool (FR-013); CHECK `wishes_remaining >= 0` |
| `doom_clock_current` | `INT NOT NULL DEFAULT 0` | Segments filled |
| `doom_clock_max` | `INT NOT NULL` | Segments to fill for a loss (FR-016); GM-set at session start |
| `status` | `TEXT NOT NULL DEFAULT 'active'`, `CHECK (status IN ('active', 'won', 'lost'))` | Set to `'won'`/`'lost'` per FR-016's win/loss condition; a terminal session is retained, not deleted, for post-session review |
| `created_by` | `UUID` FK → users | GM who started the session, per Constitution Principle III provenance convention |
| `created_at` / `updated_at` | `TIMESTAMPTZ` | |

**Validation rules**: `wishes_remaining` never goes below 0 (a `spendWish` mutation against an empty pool is rejected, per research.md's edge-case convention of rejecting insufficient-resource actions the same way elsewhere in the platform). `doom_clock_current` never exceeds `doom_clock_max`; reaching it sets `status = 'lost'` in the same mutation (FR-016), unless all Puzzle Clocks already resolved in that same action (spec.md Edge Cases — Puzzle Clock resolution is checked first).

## `world_genie_puzzle_clocks`

One row per Puzzle Clock (FR-015); a session has one or more.

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID` PK | |
| `session_id` | `UUID` FK → `world_genie_sessions.id` | |
| `label` | `TEXT NOT NULL` | GM-authored objective/station name |
| `segments_current` | `INT NOT NULL DEFAULT 0` | |
| `segments_max` | `INT NOT NULL` | GM-set at creation |
| `resolved_at` | `TIMESTAMPTZ`, nullable | Set when `segments_current` reaches `segments_max`; a resolved clock stays in the table (not deleted) so a session's history is reviewable |
| `created_at` / `updated_at` | `TIMESTAMPTZ` | |

**Validation rules**: `segments_current` never exceeds `segments_max`. The session's win condition (FR-016) is satisfied when every row for a session has a non-null `resolved_at`.

## `world_genie_resource_holdings`

A per-player ledger of Session Resources (FR-017) — not one row per resource type globally, but one row per `(session_id, actor_id, resource_type)` combination, so each player's holdings are tracked independently and a trade is just two rows' quantities changing together.

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID` PK | |
| `session_id` | `UUID` FK → `world_genie_sessions.id` | |
| `actor_id` | `UUID` FK → the holding player's actor | |
| `resource_type` | `TEXT NOT NULL` | One of the manifest's declared `sessionResources` keys (e.g. `insight`, `favor`, `essence`) |
| `quantity` | `INT NOT NULL DEFAULT 0` | CHECK `quantity >= 0` |

**Validation rules**: `quantity` never goes negative (an offered trade or a Puzzle-Clock spend that would overdraw a holding is rejected). Unique on `(session_id, actor_id, resource_type)` — one row per player per resource type per session, incremented/decremented in place rather than appending new rows.

**Lifecycle (trade)**: A `tradeSessionResource` mutation (contracts/genie-session-loop.md) is a two-step propose/accept flow (research.md R8) — a proposal is held in memory/short-lived state until the counterpart accepts, at which point both actors' holding rows update atomically in one transaction; nothing is written to `world_genie_resource_holdings` for a proposal that's never accepted.

## `world_events` (existing table, one new `event_code` value)

| `event_code` | Meaning | Payload shape (`token_event` JSON column) |
|---|---|---|
| 15 (new) | `genie_session_state` | `{ "kind": "wish_pool" \| "doom_clock" \| "puzzle_clock" \| "resource_trade", "session_id": "...", ...kind-specific fields }` |

No schema change to `world_events` itself (research.md R7) — `event_code` is already a plain `i32` with no enum constraint in the schema. Broadcast via the existing `record_world_event` function and consumed by the existing `worldEventsCreated(worldId)` subscription every world-member client already holds open.
