# Phase 1 Data Model: Items & Inventory System

All new tables live in `src/server/src/schema.rs` (Diesel) with migrations under `src/server/migrations/`, following the existing `YYYY-MM-DD-HHMMSS-NNNN_description` naming convention. All primary keys are UUIDs, matching existing convention. No human-supplied name is used as a storage identifier (FR-021).

## `world_items`

The Item itself. One row per item.

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID` PK | Storage/API identifier (FR-021) |
| `world_id` | `UUID` FK → `worlds.id` | Scopes the item; cascade-delete with world (matches actor precedent) |
| `name` | `TEXT NOT NULL` | Human-supplied; NOT unique per world (FR-019, Clarifications) |
| `description` | `TEXT`, nullable | |
| `icon_asset_id` | `UUID`, nullable FK | Reuses the existing actor-image asset shape (research.md §4); nullable per Clarifications (icon optional) |
| `created_by` | `UUID` FK → users | Provenance, per constitution Principle III convention |
| `created_at` / `updated_at` | `TIMESTAMPTZ` | |

**Indexes**: trigram GIN index on `name` (`pg_trgm`, research.md §3) to power `suggestItemName`.

**Validation rules**: `name` non-empty. No slug/uniqueness constraint on `name` (deliberate — FR-019).

**Lifecycle**: Created (DM-only, FR-002) → effects/ownership/icon edited any number of times → optionally shared/copied to another world (FR-022–027) → deleted (Owner-level, mirroring lore's delete rule since the spec reuses the ownership-block model verbatim; does NOT block on lore links or inventory references, FR-017).

## `world_item_permissions`

Direct structural mirror of `world_actor_permissions` (spec 010) / `world_lore_permissions` (spec 012).

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID` PK | |
| `item_id` | `UUID` FK → `world_items.id`, cascade delete | |
| `world_member_user_id` | `UUID` FK → user | The member being granted a level |
| `permission_level` | enum `Viewer` / `Editor` / `Owner` | Reuses the existing `ActorPermissionLevel` GraphQL enum rather than a duplicate (identical values/semantics) |
| `created_at` / `updated_at` | `TIMESTAMPTZ` | |

**Validation rules**: At most one row per `(item_id, world_member_user_id)` (unique constraint). No explicit row ⇒ default Viewer (FR-003). Row auto-deleted when the named world member is removed from the world (cascade, mirrors spec 010's FR-022).

## `world_item_effects`

Structured, system-agnostic effect data attached to an Item (FR-004/FR-004a, research.md §1).

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID` PK | |
| `item_id` | `UUID` FK → `world_items.id`, cascade delete | |
| `effect_type` | enum `heal` / `damage` / `modifier` / `attack_roll` | Extensible — new variants added by migration only, no polymorphic escape hatch |
| `formula` | `TEXT NOT NULL` | Dice/formula string, e.g. `"3d6"`, `"1d20 + STAT + MODIFIERS"`, `"2d8"`, `"-1d4"` (a negative `modifier` formula covers "detriments", research.md §1) |
| `target` | `TEXT NOT NULL` | Freeform resource/attribute name, e.g. `"Hit Points"`, `"STAT"` — not validated against any ruleset vocabulary (Assumptions) |
| `trigger_kind` | enum `on_use` / `passive`, nullable | Scaffolded per FR-004a; not evaluated/enforced by any code path in this pass |
| `sort_order` | `INT NOT NULL DEFAULT 0` | Authored display order (e.g. attack-roll before its paired damage effect) |
| `created_at` / `updated_at` | `TIMESTAMPTZ` | |

**Validation rules**: `formula` MUST be non-empty and pass a structural validity check (FR-006) — a minimal dice-grammar check (matches `\d*d\d+` optionally combined with `+`/`-` terms and bare words like `STAT`/`MODIFIERS`), not a ruleset-aware evaluator, since this spec never evaluates the formula. Rejected at the mutation layer with a clear error, never persisted invalid.

## `world_item_shares`

Direct structural mirror of `world_actor_shares` (spec 010, research.md §5).

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID` PK | |
| `item_id` | `UUID` FK → `world_items.id`, cascade delete | |
| `share_code` | `VARCHAR(32)` | Same generation scheme as `world_actor_shares.share_code` |
| `created_by` | `UUID` FK → users | |
| `revoked` | `BOOL NOT NULL DEFAULT false` | |
| `created_at` / `updated_at` | `TIMESTAMPTZ` | |

**Validation rules**: Creating a link never replaces an existing one for the same item (multiple independent, independently-revocable links are allowed, matching the actor precedent).

## `world_actor_inventory`

Join between one Actor and one Item with a quantity (FR-009–013, research.md §2).

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID` PK | |
| `actor_id` | `UUID` FK → `world_actors.id`, cascade delete | |
| `item_id` | `UUID`, nullable FK → `world_items.id`, `ON DELETE SET NULL` | Nulled (not cascade-deleted) when the referenced Item is deleted — deleting an Item MUST NOT be blocked by outstanding inventory rows (FR-017), so `ON DELETE RESTRICT` is rejected, and cascade-delete would silently vanish an Actor's inventory row instead of marking it deleted-item, which the spec's Edge Cases forbid |
| `item_name_snapshot` | `TEXT NOT NULL` | Item's `name` at the time this row was created/last touched; kept so a row can still render "Potion of Healing (deleted item)" after `item_id` goes `NULL` |
| `quantity` | `INT NOT NULL CHECK (quantity >= 0)` | Non-negative (FR-012); a row reaching 0 is deleted by application logic (FR-011), not represented as a stored zero |
| `created_at` / `updated_at` | `TIMESTAMPTZ` | |

**Validation rules**: Unique constraint on `(actor_id, item_id)` — enforces "at most one entry per distinct Item per Actor" at the DB level (Key Entities); adds use `ON CONFLICT (actor_id, item_id) DO UPDATE SET quantity = world_actor_inventory.quantity + excluded.quantity` (research.md §2). `quantity < 0` is rejected before any write (FR-012).

## Reused, unmodified entities

- **`world_actors`** (spec 010): gains an inventory via the new `world_actor_inventory` join table; no schema change to `world_actors` itself. Also a valid `[[...]]` link target from lore, unchanged.
- **`world_members`** / world role resolution (spec 009/010): supplies the assignable-subject pool for `world_item_permissions` and the "DM = Owner or GM role" authorization check, via a generalized/duplicated version of `auth/actor_permissions.rs::is_dm_of_world` (implementation detail for tasks.md).
- **`world_lore_entries` / `world_lore_links`** (spec 012, once implemented): `world_items` becomes a third valid `target_kind` alongside `lore_entry`/`actor` in the existing link-resolution table (FR-014/015/016) — no new table required on the lore side, just a new nullable `target_item_id` column and enum variant on `world_lore_links`.

## Entity relationship summary

```text
World ──1:N── Item ──1:N── ItemEffect
                │
                ├──1:N── ItemPermission ──N:1── WorldMember
                │
                └──1:N── ItemShare

Actor ──1:N── ActorInventoryEntry ──N:1── Item (nullable on item delete, item_name_snapshot retained)

LoreEntry ──1:N── LoreLink (source) ──N:1── LoreEntry (target, nullable)
                                    ├──N:1── Actor (target, nullable)
                                    └──N:1── Item (target, nullable)   # new in this spec
```
