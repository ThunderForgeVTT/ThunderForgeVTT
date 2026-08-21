# Phase 1 Data Model: Canvas-Native Token Authoring & Scene-Switch Loading Feedback

## Schema change: `tokens` table gains four columns (new migration required)

Per research.md §1-3, this feature unifies token authoring onto the existing scene-scoped `tokens` table (`src/server/src/schema.rs:254-266`) rather than introducing a new table. `x`, `y`, `rotation`, `scale`, `actor_id`, `metadata` already exist and are unchanged.

**New migration** (`src/server/migrations/<timestamp>_add_ownership_and_photo_to_tokens/`):

```sql
-- up.sql
ALTER TABLE tokens ADD COLUMN owner_user_id UUID NULL REFERENCES users(id);
ALTER TABLE tokens ADD COLUMN is_primary BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE tokens ADD COLUMN photo_url TEXT NULL;
ALTER TABLE tokens ADD COLUMN health INTEGER NULL;
ALTER TABLE tokens ADD COLUMN max_health INTEGER NULL;

CREATE UNIQUE INDEX tokens_one_primary_per_owner_per_scene
  ON tokens (scene_id, owner_user_id)
  WHERE is_primary;
```

```sql
-- down.sql
DROP INDEX IF EXISTS tokens_one_primary_per_owner_per_scene;
ALTER TABLE tokens DROP COLUMN IF EXISTS max_health;
ALTER TABLE tokens DROP COLUMN IF EXISTS health;
ALTER TABLE tokens DROP COLUMN IF EXISTS photo_url;
ALTER TABLE tokens DROP COLUMN IF EXISTS is_primary;
ALTER TABLE tokens DROP COLUMN IF EXISTS owner_user_id;
```

### `tokens` table — full shape after this feature

| Column | Type | Status | Relevance |
|---|---|---|---|
| `token_id` | UUID (PK) | unchanged | Identity |
| `scene_id` | UUID (FK → `scenes`) | unchanged | Scoping, GM/scene-owner authorization (reused verbatim) |
| `actor_id` | nullable UUID | unchanged | Existing hook toward a future character-sheet/actor-stats link (out of scope here; not touched) |
| `x`, `y` | Float8 | unchanged | Canvas position — now the single field GM canvas-drag (FR-001-004) and player canvas-drag (FR-009) both write, through separate mutations |
| `rotation` | Float8 | unchanged | Facing — GM-only (FR-007), already accepted by `update_token` |
| `scale` | Float8 | unchanged | Size — GM-only (FR-006), already accepted by `update_token`; client enforces whole-grid-cell-multiple snapping before sending (no new DB constraint) |
| `metadata` | nullable Jsonb | unchanged | Untouched by this feature |
| `owner_user_id` | nullable UUID (FK → `users`) | **new** | The player who controls this token — either their primary token or one the GM additionally granted (User Story 3) |
| `is_primary` | Bool, default false | **new** | True for exactly one token per `(scene_id, owner_user_id)` — enforced by partial unique index |
| `photo_url` | nullable Text | **new** | Player-/GM-editable avatar override; falls back to the existing deterministic Dicebear URL (`TokenPanel.tsx`'s `getTokenAvatar`) when null |
| `health`, `max_health` | nullable Int4 | **new** | Ported concept from the retired `world_tokens` table, kept as a `TokenPanel.tsx` responsibility |
| `created_at`, `updated_at` | Timestamp | unchanged | Existing provenance |

### `world_tokens` table — retired, not migrated, not read

Per research.md §1, `world_tokens` (`schema.rs:408-423`) is left in place as inert legacy data (no clean world→scene mapping exists to migrate it). No code path in this feature reads or writes it after `TokenPanel.tsx` is rewired. No migration drops it — out of scope; a future cleanup spec may formally remove it once confirmed nothing depends on it.

## Authorization surfaces (Constitution Principle III — reused/extended patterns)

- **`create_token` / `delete_token`** (existing, `mutations_tokens.rs`): unchanged, scene-owner (GM) only.
- **`update_token`** (existing, extended): scene-owner (GM) only, unchanged authorization filter (`tokens::scene_id.eq_any(scenes::table.filter(scenes::owner_id.eq(user_id))...)`), now additionally accepting `owner_user_id`, `is_primary`, `photo_url`, `health`, `max_health` as settable fields alongside existing ones.
- **`move_own_token`** (new): filters `tokens::owner_user_id.eq(user_id)` at the DB level (ADR-033's "ownership enforced at the DB level" pattern) in addition to requiring the requester be a member of the token's scene's world (reusing the existing world-membership check pattern, not a new one); touches only `x`, `y`.
- **`set_own_primary_token_photo`** (new): filters `tokens::owner_user_id.eq(user_id).and(tokens::is_primary.eq(true))`; touches only `photo_url`.

No new authorization *mechanism* is introduced — both new mutations reuse the existing "filter at the Diesel query level" pattern already established by `mutations_walls.rs`/`mutations_tokens.rs`'s scene-owner check, applied to a new column (`owner_user_id`) instead of `scenes.owner_id`.

## Client-side (RxDB / engine) — no new persisted shape beyond the above

- `apps/web/src/engine/world/sync/tokens.ts` gains the five new fields to its existing sync shape (no new collection).
- `TokenPanel.tsx` is rewired from the `world_tokens` RxDB collection to the `tokens` one; its UI responsibilities shrink to: bulk create/delete (GM), health editing (GM), primary-token photo editing (the owning player, via `set_own_primary_token_photo`, or GM via `update_token`).
- No new engine ECS component beyond what `wall.rs`/`shape.rs` already establish as the handle-rendering pattern (marker components for resize/rotate handles, mirroring `WallHandle`).

## Scene-switch loading/error state (User Story 4) — client-only, not persisted

Not a database entity. A client-side state machine in `WorldPage.tsx` (or a small extracted hook) with three states: `loading` (while the four per-scene loaders + background image fetch are in flight for the current `sceneId`), `ready` (all succeeded), `error` (any required load failed — in particular the background image), plus a `retry()` action that re-invokes the same loaders for the same `sceneId`. No schema, no migration.
