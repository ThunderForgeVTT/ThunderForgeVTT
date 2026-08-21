# ADR-040: Unify Token Backing Store onto the Scene-Scoped `tokens` Table

**Date:** 2026-08-21
**Status:** ACCEPTED
**Participants:** ThunderForgeVTT Team

---

## Problem Statement

Two unrelated token tables exist in this codebase today:

1. `world_tokens` (world-scoped, `ADR-033`'s original design) — read/written only by `TokenPanel.tsx` via `moveToken`/`createWorldToken`/`deleteWorldToken`.
2. `tokens` (scene-scoped, created by a later migration, `2026-05-05-010001-0002_create_tokens_table`) — the table the Bevy canvas engine actually renders and syncs (`apps/web/src/engine/world/sync/tokens.ts`, `src/engine/src/systems/selection.rs`'s `handle_token_drag`).

Moving a token via the panel and dragging a token on the canvas are two unrelated rows in two unrelated tables. Spec 004 (canvas-native token authoring: drag/resize/rotate, per-player primary token, GM-grantable additional control) requires the canvas and the panel to agree on one token's state (FR-005) — impossible while they're backed by different tables.

## Decision

Standardize on the scene-scoped `tokens` table as the single source of truth for tokens, going forward. Extend it with the columns spec 004 needs (`owner_user_id`, `is_primary`, `photo_url`, `health`, `max_health`). Rewire `TokenPanel.tsx` off `world_tokens` onto `tokens`. Leave `world_tokens` in place, unread, as retired legacy data — no automatic migration of its rows, since `world_tokens` is world-scoped and `tokens` is scene-scoped with no clean 1:1 mapping between the two.

This ADR **partially supersedes ADR-033** (`20260505-033-token_data_model_and_ownership.md`): ADR-033's `world_tokens` schema and its `move_token`/ownership-filter code sample are no longer the active design for token authoring — `tokens` is. ADR-033's general patterns (DB-level ownership enforcement, event-driven mutation logging via `world_events`) remain in force and now apply to `tokens` instead.

### Why not migrate `world_tokens` data into `tokens`?

`world_tokens` rows carry a `world_id` but no `scene_id` — there is no scene to assign a historical row to without guessing. This project is pre-release; per spec 004's Assumptions, no production `world_tokens` data is at stake. If any ever mattered, migrating it would be a deliberate, manual, follow-up exercise — not an automatic step of this change.

### Schema change

```sql
ALTER TABLE tokens ADD COLUMN owner_user_id UUID NULL REFERENCES users(id);
ALTER TABLE tokens ADD COLUMN is_primary BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE tokens ADD COLUMN photo_url TEXT NULL;
ALTER TABLE tokens ADD COLUMN health INTEGER NULL;
ALTER TABLE tokens ADD COLUMN max_health INTEGER NULL;

CREATE UNIQUE INDEX tokens_one_primary_per_owner_per_scene
  ON tokens (scene_id, owner_user_id)
  WHERE is_primary;
```

`x`, `y`, `rotation`, `scale`, `actor_id`, `metadata` on `tokens` are unchanged — `rotation`/`scale` already existed and were already accepted by `update_token`, just never authored from the canvas UI until spec 004.

### Authorization

Unchanged pattern from ADR-033/Principle III — ownership enforced at the DB query level, not in application code:

- `create_token`/`update_token`/`delete_token` (GM/scene-owner only): unchanged filter, `tokens.scene_id.eq_any(scenes owned by requester)`.
- `move_own_token`, `set_own_primary_token_photo` (new, player-facing): filtered by `tokens.owner_user_id = <requester>` (and `is_primary = true` for the photo mutation) — the same DB-level-filter discipline, applied to a new column.

### Consequences

**Positive**:
- One token, one source of truth — canvas and panel can never disagree.
- No player-owns-token authorization mechanism had to be invented; it reuses the existing DB-filter pattern.
- `rotation`/`scale` already existing meant no schema work was needed for those two fields.

**Negative**:
- `world_tokens` becomes dead code/dead data — a future cleanup spec should formally drop it once confirmed nothing depends on it.
- `TokenPanel.tsx` needed real rework (not cosmetic) to move off `world_tokens`'s RxDB collection and mutations.

## Related ADRs

- ADR-033: Token Data Model & Ownership (partially superseded — `world_tokens` portion only)
- ADR-009 / ADR-013: ownership/authorization enforcement conventions, unchanged and reused

## References

- Spec 004: `specs/004-token-canvas-authoring/` (research.md §1, §3, §4; data-model.md; plan.md Constitution Check)
