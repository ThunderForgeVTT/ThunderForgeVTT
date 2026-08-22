# Phase 1 Data Model: World Staging Route and Actor Ownership

## Existing entities reused, one column reinterpreted

### World Actor (`world_actors` table, existing — no column added/removed)

Already fully modeled (spec 009). This feature changes how one existing column is *used*, not its shape:

| Column | Change |
|---|---|
| `owned_by` | **Reinterpreted.** Still `NOT NULL Uuid`, still populated at creation (set to the creating DM's `user_id`), still exposed read-only as `ownedBy` on `GraphQLWorldActor`. No longer consulted by any authorization check — that responsibility moves entirely to the new `world_actor_permissions` table plus the DM-always-implicit-Owner rule (research.md §4). |
| `is_npc` | Unchanged — remains the canonical PC/NPC classification flag (FR-008). |
| `scene_id` | Unchanged (`NOT NULL`) — a staging-page-created actor is assigned the world's earliest-created scene by default (research.md §6). |

All other columns (`id`, `world_id`, `actor_type`, `game_system_id`, `label`, `created_by`, `is_public`, `created_at`, `updated_at`) are unchanged.

### World Actor System Data (`world_actor_system_data` table, existing — unchanged)

The existing cascaded-sub-data table (`ability_data`, `resource_data`, `proficiency_data`, `trait_data`, `spell_data`, all `Jsonb`) is what FR-026's "abilities/items/actor-specific lore" deep-copy clones — this feature treats every row keyed by an actor's `actor_id` as part of that actor's cascaded data, regardless of which JSONB column a given game system stores what in. No schema change; a copy operation duplicates each row with a new `id` and the destination actor's `actor_id`.

### World Member (`world_members` table, existing — unchanged)

Reused as the pool of assignable subjects in an actor's ownership block (`role` values `Owner`/`GM`/`Player`, `Owner`+`GM` both meaning "DM" per FR-021, research.md §3).

## New tables

### `world_actor_permissions` (new)

The "ownership block" (FR-013). One row per (actor, world member) with an explicit, non-default permission level.

| Column | Type | Notes |
|---|---|---|
| `id` | Uuid | PK |
| `actor_id` | Uuid | FK → `world_actors(id)`, `ON DELETE CASCADE` (deleting an actor deletes its permission rows) |
| `user_id` | Uuid | FK → `users(id)`, `ON DELETE CASCADE` (deleting a user account deletes their grants) |
| `level` | Varchar(16) | One of `Viewer` \| `Editor` \| `Owner` (app-level check, mirrors `world_members.role`'s existing Varchar-not-DB-enum convention) |
| `created_at` / `updated_at` | Timestamp | |

Constraints:
- `UNIQUE (actor_id, user_id)` — at most one explicit level per member per actor; re-assigning is an UPSERT on this pair, not a new row (setting a member's level via `setActorPermission` updates the existing row if present).
- No `UNIQUE` on `(actor_id)` alone for `level = 'Owner'` — multiple simultaneous Owner rows for the same actor are valid (clarification: "Allow multiple Owners").

Absence rule: a `(actor_id, user_id)` pair with no row means that member has default `Viewer` access to that actor (FR-016) — "no row" is a valid, expected, common state (e.g., every NPC with no explicit grants at all), not an error or a state requiring a placeholder row.

Cleanup rules:
- Actor deleted → all its `world_actor_permissions` rows deleted (DB-level `ON DELETE CASCADE` via `actor_id`).
- World member removed (leaves or is kicked) → all `world_actor_permissions` rows for that `(user_id, any actor in that world)` are deleted by the removal mutation, in the same transaction, via an explicit `DELETE ... WHERE user_id = :removed AND actor_id IN (SELECT id FROM world_actors WHERE world_id = :world)` (research.md §7 — not a DB-level FK cascade, since the relationship between "world membership" and this table is via `world_id` on the joined `world_actors` row, not a direct FK).

### `world_actor_shares` (new)

An actor's shareable link (FR-023, User Story 5). Modeled directly on `world_invites`' existing shape, minus the usage-cap/expiry fields (share links are persistent/uncapped per spec Assumptions — the only lifecycle event is revocation).

| Column | Type | Notes |
|---|---|---|
| `id` | Uuid | PK |
| `actor_id` | Uuid | FK → `world_actors(id)`, `ON DELETE CASCADE` |
| `share_code` | Varchar(32) | Unique, opaque random token (same generation approach as `world_invites.invite_code`) — never encodes the source world/actor id |
| `created_by` | Uuid | FK → `users(id)` — who generated it (must have held Owner-level access at creation time; not re-checked continuously) |
| `revoked` | Boolean | `NOT NULL DEFAULT false` — set `true` by `revokeActorShareLink` |
| `created_at` / `updated_at` | Timestamp | |

A share link with `revoked = true`, or whose `actor_id` no longer resolves (actor deleted, cascading this row away entirely — so in practice "no longer available" is really just "row not found"), both surface as the same "no longer available" state to a viewer (FR-024's edge case) — the distinction between revoked-but-present and cascaded-away is not exposed to the client.

## New client-only concept: effective actor permission

Not a new table — a computed value. `GraphQLWorldActor` gains one server-resolved field, `myPermissionLevel: ActorPermissionLevel!` (enum: `VIEWER` | `EDITOR` | `OWNER`), computed per-request as:

1. Caller is DM (Owner or GM role) of the actor's world → `OWNER` (FR-017).
2. Else, caller has an explicit `world_actor_permissions` row for this actor → that row's `level`.
3. Else → `VIEWER` (FR-016, default).

The frontend gates edit controls (show/hide "Edit," enable/disable the `/edit` route's save action) purely by reading this field — it never re-derives permission logic client-side, keeping the server authoritative (Principle III) exactly as `require_actor_permission` (research.md §4) enforces server-side on every mutation regardless of what the client shows.

## Relationships (new edges only; existing edges unchanged)

```
World 1──* WorldActor (existing)
WorldActor 1──* ActorSystemData (existing)
WorldActor 1──* WorldActorPermission (new) ──* User (existing)
WorldActor 1──* WorldActorShare (new) ──1 User (created_by, existing)
World 1──* WorldMember (existing) ── (app-level join, no direct FK) ──* WorldActorPermission cleanup on removal
```

## State transitions

- **Actor permission entry**: absent (default Viewer) → explicit `Viewer`/`Editor`/`Owner` (via `setActorPermission`, DM-only) → back to absent (via `removeActorPermission`, DM-only) or deleted en masse (member removed from world, or actor deleted).
- **Share link**: created (`revoked = false`) → revoked (`revoked = true`, terminal — no "un-revoke" specified) → (independently) gone entirely if the source actor is deleted.
- **Copied actor**: created via `copySharedActorToWorld` as a brand-new `world_actors` row (fresh `id`, `world_id` = destination, `scene_id` = destination world's default scene per research.md §6, `owned_by` = copier's `user_id`, zero `world_actor_permissions` rows) plus cloned `world_actor_system_data` rows (fresh `id`s, same content, `actor_id` = the new actor). From the moment of creation it is a fully ordinary, independent actor — no different from one created via `createActor`, and no field anywhere links it back to its source.
