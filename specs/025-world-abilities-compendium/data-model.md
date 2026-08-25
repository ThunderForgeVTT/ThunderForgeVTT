# Data Model: World Abilities Compendium

Derived from [spec.md](./spec.md)'s Key Entities, mirroring spec 013's item
schema (see [research.md](./research.md) §2 for why duplication is preferred
over generalizing items and abilities into a shared abstraction).

All DDL below follows the exact conventions the item tables established —
`gen_random_uuid()` PK defaults on the primary/child content tables, app-supplied
`Uuid::now_v7()` PKs on the permission/share tables, `ON DELETE CASCADE` to the
owning row, and `TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP` audit columns.

## Entity overview

| Entity | Table | Mirrors |
|---|---|---|
| Ability | `world_abilities` | `world_items` |
| Ability Effect | `world_ability_effects` | `world_item_effects` |
| Ability permission entry | `world_ability_permissions` | `world_item_permissions` |
| Ability Share Link | `world_ability_shares` | `world_item_shares` |
| Known Ability Entry | `world_actor_abilities` | `world_actor_inventory` |
| Ability Classification | *(no table — fixed enum)* | — |
| Ability Presentation Facet | *(no table — system manifest)* | — |

---

## 1. `world_abilities`

The core entity (FR-001, FR-003, FR-006).

```sql
CREATE TABLE world_abilities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    world_id UUID NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    classification VARCHAR(16) NOT NULL
        CHECK (classification IN ('spell', 'feat', 'power', 'talent')),
    gm_only BOOLEAN NOT NULL DEFAULT FALSE,
    created_by UUID NOT NULL REFERENCES users(id),
    updated_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX world_abilities_world_id_idx ON world_abilities(world_id);
CREATE INDEX world_abilities_name_trgm_idx ON world_abilities USING GIN (name gin_trgm_ops);
```

**Notes / deliberate differences from `world_items`:**

- **`updated_by` is present.** `world_items` has only `created_by`, but spec 025
  FR-027 explicitly requires `created_by`/`updated_by` provenance per
  Constitution Principle III. Adding it here is a small correctness improvement
  over the template, not a divergence for its own sake.
- **`classification` is `NOT NULL`** with a DB CHECK, matching how
  `world_item_effects.effect_type` constrains its enum. The four values are the
  fixed, portable set (FR-009); systems re-label them via facets (FR-010) but
  cannot add to them (Non-Goals).
- **`gm_only` is the visibility control (Clarification, Session 2026-08-25).**
  Deliberately a column on the ability, *not* a level in the ownership block —
  `ActorPermissionLevel`'s lowest value is `Viewer`, which is also its default
  for a member with no row, so the permission model structurally **cannot**
  express "hidden". The ownership block governs edit rights; `gm_only` governs
  visibility. This mirrors `scenes.hidden`, the only existing precedent for
  hiding content from non-DMs, and avoids adding a fourth level to an enum that
  actors, lore, and items all share. Every read path for a non-DM must filter on
  `gm_only = false` — see the query-surface table below.
- **No `icon_asset_id`** — abilities have no image in this pass.
- **No uniqueness on `name`** — deliberate, FR-006. The `gin_trgm_ops` index
  backs FR-007's advisory "did you mean?" query, reusing the `pg_trgm`
  extension already enabled by spec 013's `enable_pg_trgm` migration (no new
  extension migration needed).

## 2. `world_ability_effects`

Structured, inert authored data (FR-015, FR-016, FR-019, FR-020).

```sql
CREATE TABLE world_ability_effects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ability_id UUID NOT NULL REFERENCES world_abilities(id) ON DELETE CASCADE,
    effect_type VARCHAR(16) NOT NULL
        CHECK (effect_type IN ('heal', 'damage', 'modifier', 'attack_roll')),
    formula TEXT NOT NULL,
    target TEXT NOT NULL,
    trigger_kind VARCHAR(16)
        CHECK (trigger_kind IS NULL OR trigger_kind IN ('on_use', 'passive')),
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX world_ability_effects_ability_id_idx ON world_ability_effects(ability_id);
```

Structurally identical to `world_item_effects` — same four effect types
(FR-016 requires *at least* parity), same free-text `formula`/`target`, same
nullable `trigger_kind` scaffold (FR-020), same `sort_order` display ordering.
Deliberately identical so a future resolution engine (spec 014's dice crate)
can consume item and ability effects through one code path.

## 3. `world_ability_permissions`

The ownership block (FR-024, FR-025, FR-026).

```sql
CREATE TABLE world_ability_permissions (
    id UUID PRIMARY KEY,
    ability_id UUID NOT NULL REFERENCES world_abilities(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    level VARCHAR(16) NOT NULL CHECK (level IN ('Viewer', 'Editor', 'Owner')),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (ability_id, user_id)
);
CREATE INDEX world_ability_permissions_ability_id_idx ON world_ability_permissions(ability_id);
CREATE INDEX world_ability_permissions_user_id_idx ON world_ability_permissions(user_id);
```

- PK has **no** default — app supplies `Uuid::now_v7()`, matching
  `world_item_permissions`.
- `UNIQUE (ability_id, user_id)` is the upsert conflict target for
  `setAbilityPermission`.
- `level` reuses the existing `ActorPermissionLevel` Rust enum
  (`Viewer`/`Editor`/`Owner`, capitalized to match the CHECK) — **not** a new
  ability-specific enum. Items already reuse it; a third copy would be waste.
- Absence of a row means Viewer (FR-024's default); a DM is always Owner
  regardless of rows (resolved in code, never stored).

## 4. `world_ability_shares`

Share links (FR-032, FR-036, FR-037).

```sql
CREATE TABLE world_ability_shares (
    id UUID PRIMARY KEY,
    ability_id UUID NOT NULL REFERENCES world_abilities(id) ON DELETE CASCADE,
    share_code VARCHAR(32) NOT NULL UNIQUE,
    created_by UUID NOT NULL REFERENCES users(id),
    revoked BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX world_ability_shares_ability_id_idx ON world_ability_shares(ability_id);
```

- PK app-supplied (`Uuid::now_v7()`); `share_code` is a random **v4**-derived
  20-char uppercase code, `UNIQUE`.
  - ⚠️ The v4 (not v7) derivation is load-bearing. Spec 005 found and fixed a
    real collision bug where invite codes derived from a **v7** UUID's first
    hex characters collided whenever two were generated in the same millisecond
    (v7 front-loads a timestamp). `generate_share_code()` already uses v4
    correctly — do not "optimize" it to v7.
- `revoked` is a soft flag, never a delete — FR-036 requires a revoked link to
  render a distinct "no longer available" state, which a deleted row could not.
- **FR-037 (no enumeration) is satisfied structurally**: there is no
  world-scoped or global "list shares" query in the item precedent, and none is
  added here. A share is reachable only by possessing its code.

## 5. `world_actor_abilities`

Actor attachment — "this NPC knows this ability" (FR-021, FR-022, FR-023).

```sql
CREATE TABLE world_actor_abilities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    actor_id UUID NOT NULL REFERENCES world_actors(id) ON DELETE CASCADE,
    ability_id UUID REFERENCES world_abilities(id) ON DELETE SET NULL,
    ability_name_snapshot TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (actor_id, ability_id)
);
CREATE INDEX world_actor_abilities_actor_id_idx ON world_actor_abilities(actor_id);
CREATE INDEX world_actor_abilities_ability_id_idx ON world_actor_abilities(ability_id);
```

**This is the load-bearing table for FR-023's "deleted ability" behavior**, and
it copies `world_actor_inventory`'s exact pattern:

- `ability_id` is **nullable** with `ON DELETE SET NULL` — deleting an ability
  must never be blocked by actors knowing it (FR-023, and the same rule lore
  links follow). The row survives with a null reference.
- `ability_name_snapshot` is `NOT NULL` and captured at attach time, so a row
  whose `ability_id` went null can still render *"Fireball (deleted)"* rather
  than an unidentifiable orphan.
- `UNIQUE (actor_id, ability_id)` enforces FR-021's no-duplicates rule at the
  DB level (re-attaching is a no-op upsert, not a second row).
  - ⚠️ Postgres treats NULLs as distinct in a UNIQUE constraint, so multiple
    deleted-ability rows per actor are permitted — which is correct: two
    different deleted abilities must both remain listed.
- **No `quantity` column.** This is the deliberate difference from
  `world_actor_inventory`: an actor either knows an ability or does not
  (Non-Goals — no slots, charges, or preparation counts).

## 6. `world_lore_links` — additive change

Mirrors spec 013's `add_item_target_to_world_lore_links` migration exactly,
widening the existing 3-way target to 4-way (FR-028, FR-031).

```sql
ALTER TABLE world_lore_links
    ADD COLUMN target_ability_id UUID REFERENCES world_abilities(id) ON DELETE SET NULL;
CREATE INDEX world_lore_links_target_ability_id_idx ON world_lore_links(target_ability_id);

ALTER TABLE world_lore_links DROP CONSTRAINT world_lore_links_target_kind_check;
ALTER TABLE world_lore_links ADD CONSTRAINT world_lore_links_target_kind_check
    CHECK (target_kind IN ('lore_entry', 'actor', 'item', 'ability', 'unresolved'));

ALTER TABLE world_lore_links DROP CONSTRAINT world_lore_links_check;
ALTER TABLE world_lore_links ADD CONSTRAINT world_lore_links_check
    CHECK (
        (CASE WHEN target_lore_entry_id IS NOT NULL THEN 1 ELSE 0 END) +
        (CASE WHEN target_actor_id      IS NOT NULL THEN 1 ELSE 0 END) +
        (CASE WHEN target_item_id       IS NOT NULL THEN 1 ELSE 0 END) +
        (CASE WHEN target_ability_id    IS NOT NULL THEN 1 ELSE 0 END) <= 1
    );
```

`ON DELETE SET NULL` (not RESTRICT/CASCADE) is required by FR-031 — the link row
must survive and render unresolved.

**Resolution precedence** (FR-030): the existing fixed order is
lore entry → actor → item. Abilities append as the **last** resolution target,
making it lore entry → actor → item → ability. Appending rather than inserting
guarantees no existing link changes meaning — a title that resolves to an item
today keeps resolving to that item. See research.md §4.

---

## Non-table entities

### Ability Classification (fixed enum, FR-009)

A closed set shared by every game system, stored as the `classification` string
on `world_abilities`. Not a table — adding a classification is a code + migration
change, deliberately (Non-Goals exclude a GM-extensible taxonomy).

| DB value | Built-in default label |
|---|---|
| `spell` | Spell |
| `feat` | Feat |
| `power` | Power |
| `talent` | Talent |

Represented as an `async_graphql::Enum` (`AbilityClassification`) with
`as_db_str`/`from_db_str`, mirroring `ItemEffectType`'s round-trip convention.
On an unrecognized DB string, fall back to `Spell` (mirroring
`GraphQLItemEffect`'s fallback-to-`Modifier` behavior) rather than erroring.

### Ability Presentation Facet (system manifest, FR-010..FR-013)

Per-system display labels, carried in the system manifest rather than the
database — they are a property of the *game system*, not of any world's data.
Optional; absent facets fall back to the built-in labels above (FR-011).

Full design, the manifest shape, and the ADR-027 amendment question are covered
in [research.md](./research.md) §1.

---

## Relationships

```text
worlds ──1:N──> world_abilities ──1:N──> world_ability_effects
                      │
                      ├──1:N──> world_ability_permissions ──N:1──> users
                      ├──1:N──> world_ability_shares ──N:1──> users
                      ├──0:N──< world_actor_abilities >──N:1── world_actors
                      └──0:N──< world_lore_links (target_ability_id)
```

- `world_abilities` → `worlds`: CASCADE (deleting a world removes its abilities).
- effects / permissions / shares → `world_abilities`: CASCADE (children die with
  the ability).
- `world_actor_abilities` and `world_lore_links` → `world_abilities`: SET NULL
  (references survive deletion as tombstones — FR-023, FR-031).

## GM-only filtering: every read surface

`gm_only` is only as good as the least-careful query. FR-024b requires a GM-only
ability to be absent from **every** non-DM-reachable surface, so each of these
must filter `gm_only = false` unless the caller is a DM of the ability's world.
A miss on any one row is a content leak, not a cosmetic bug.

| Surface | Query / path | Filter required |
|---|---|---|
| Compendium list + search | `worldAbilities` | `gm_only = false` for non-DM |
| Detail view | `ability` | reject for non-DM (FR-025) |
| "Did you mean?" suggestions | `suggestAbilityName` | `gm_only = false` for non-DM |
| Actor known-abilities | `actorAbilities` | join-filter for non-DM (FR-023) |
| Attach-ability catalog | `worldAbilities` (reused) | inherited from the list filter |
| Lore link autocomplete | `lore_link_targets_impl` | `gm_only = false` for non-DM |
| Lore link resolution | `markdown/links.rs` cascade | skip GM-only for non-DM reader |
| Ability backlinks | `GraphQLAbility.linkedFromLore` | unreachable — detail already denied |
| Share preview | `sharedAbility` | see contracts/ability-share.md |

**Rendering is viewer-dependent.** Lore `rendered_html` is re-rendered on every
read (not served from a stored snapshot), so link resolution can and must be
evaluated against the current viewer. A GM and a player reading the same lore
entry can legitimately see the same `[[Name]]` resolve differently — one to the
ability, one as an unresolved span.

## Deterministic name resolution

FR-006 permits duplicate ability names, but `[[Title]]` cannot express which one
is meant. Resolution therefore orders explicitly:

```sql
... WHERE world_id = $1 AND name ILIKE $2
    AND (NOT gm_only OR $viewer_is_dm)
ORDER BY created_at ASC
LIMIT 1
```

The `ORDER BY` is load-bearing, not decorative — without it Postgres may return
either row, so the same link can resolve to a different ability between reads.
The existing item resolver has this same latent bug and should get the same fix.

## Validation rules

| Rule | Source | Enforced where |
|---|---|---|
| Effect formula non-empty and contains ≥1 alphanumeric | FR-018 | `validate_formula` (server, before write) |
| Effect target not whitespace-only | FR-018 | `validate_target` (server, before write) |
| `classification` ∈ fixed set | FR-009 | DB CHECK + GraphQL enum |
| `effect_type` ∈ fixed set | FR-016 | DB CHECK + GraphQL enum |
| `level` ∈ Viewer/Editor/Owner | FR-024 | DB CHECK + `ActorPermissionLevel` |
| `gm_only` defaults to false on create | FR-024a | DB `DEFAULT FALSE` |
| Only a DM may set/clear `gm_only` | FR-024c | Resolver guard (`setAbilityGmOnly`) |
| Every non-DM read path filters `gm_only` | FR-024b | Per-query (see table above) |
| Duplicate-name resolution is deterministic | FR-030a | `ORDER BY created_at ASC` |
| One ability per actor (no duplicates) | FR-021 | `UNIQUE (actor_id, ability_id)` |
| At most one link target set | FR-028 | `world_lore_links_check` |
| Name uniqueness NOT enforced | FR-006 | *(deliberately absent)* |

Note: spec 013's copy path clones effects **without** re-running
`validate_formula`. Since the source was validated at authoring time this is
safe today, but the ability copy path should re-validate anyway — see
research.md §3's list of template bugs not to inherit.

## Moderation integration

Abilities become a moderatable entity type, mirroring items:

- New `ModerationEntityType::WorldAbility ↔ "world_ability"`.
- `worldAbilities` list query filters through `moderation::filter_visible`.
- `ability` detail query returns a moderated placeholder when
  `moderation::effective_status` is `Some`.
- `sharedAbility` blocks entirely on a moderated ability — closing the same
  bypass spec 013's `shared_item_is_unavailable_once_moderation_disabled` test
  guards.

This is required, not optional: without it, share links would be a moderation
bypass for exactly the content type the DMCA guardrail is concerned with.
