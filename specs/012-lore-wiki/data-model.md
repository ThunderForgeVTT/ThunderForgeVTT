# Phase 1 Data Model: World Lore Wiki

All new tables live in `src/server/src/schema.rs` (Diesel) with migrations under `src/server/migrations/`, following the existing `YYYY-MM-DD-HHMMSS-NNNN_description` naming convention. All primary keys are UUIDs (`uuid` v7, matching existing convention in `src/server/Cargo.toml`); no human-supplied name is ever used as a storage/URL identifier except via the derived `slug` column, per FR-011/FR-012.

## `world_lore_entries`

The wiki page itself. One row per lore entry.

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID` PK | Never exposed in shareable URLs (slug is used instead) |
| `world_id` | `UUID` FK → `worlds.id` | Scopes the entry; cascade-delete with world (matches actor precedent) |
| `title` | `TEXT NOT NULL` | Human-supplied |
| `slug` | `TEXT NOT NULL` | Urlified from `title`; unique on `(world_id, slug)`; disambiguated with numeric suffix on collision (FR-013) |
| `content` | `TEXT NOT NULL` | Denormalized current content (mirrors latest `world_lore_revisions` row), capped at 25 MB (FR-010a) |
| `current_revision_id` | `UUID` FK → `world_lore_revisions.id`, nullable until first save | |
| `created_by` | `UUID` FK → users | Provenance, per constitution Principle III convention |
| `created_at` / `updated_at` | `TIMESTAMPTZ` | |

**Validation rules**: `title` non-empty; `content` ≤ 25 MB (FR-010a, rejected before write); `slug` regenerated (with disambiguation) whenever `title` changes (FR-014) — old slug does not persist as a redirect table in this pass (spec allows "redirect or 404-gracefully"; 404-gracefully is the simpler default, noted here for tasks.md to make concrete).

**Lifecycle**: Created (DM-only, FR-002) → edited any number of times (each edit appends a `world_lore_revisions` row, FR-016) → optionally restored to a prior revision (appends a new revision, FR-018) → deleted (Owner-level, FR-021; cascades link/permission/image rows per below, but does NOT block on other entries' dangling links, FR-020).

## `world_lore_permissions`

Direct structural mirror of `world_actor_permissions` (spec 010) — same enum, same defaulting rule, same DM-implicit-Owner rule, generalized to lore entries instead of actors.

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID` PK | |
| `lore_entry_id` | `UUID` FK → `world_lore_entries.id`, cascade delete | |
| `world_member_user_id` | `UUID` FK → user | The member being granted a level |
| `permission_level` | enum `Viewer` / `Editor` / `Owner` | Mirrors `ActorPermissionLevel` — likely reuse that same GraphQL enum type rather than defining a duplicate, since the values and semantics are identical (see contracts/graphql-lore.md) |
| `created_at` | `TIMESTAMPTZ` | |
| `updated_at` | `TIMESTAMPTZ` | Matches the existing `world_actor_permissions` precedent (`src/server/migrations/2026-08-22-191638-0000_add_world_actor_permissions`), which carries both columns; bumped whenever `setLorePermission` upserts an existing row's level |

**Validation rules**: At most one row per `(lore_entry_id, world_member_user_id)` (unique constraint) — re-assigning a level updates the existing row rather than inserting a duplicate. No explicit row ⇒ default Viewer (FR-016's lore equivalent, spec 012 FR-003). Row auto-deleted when the named world member is removed from the world (cascade, mirrors FR-022 from spec 010, reused verbatim per Assumptions).

## `world_lore_links`

Persisted resolution of every `[[...]]` occurrence found in an entry's content **at last save time** (research.md §2) — drives the "linked from" backlink list (FR-006) without a live re-scan on every read.

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID` PK | |
| `source_lore_entry_id` | `UUID` FK → `world_lore_entries.id`, cascade delete | The entry whose content contains the link |
| `raw_title` | `TEXT NOT NULL` | The literal `[[Title]]` text at save time, kept for display/debugging of unresolved links |
| `target_kind` | enum `lore_entry` / `actor` / `unresolved` | |
| `target_lore_entry_id` | `UUID` FK → `world_lore_entries.id` `ON DELETE SET NULL`, nullable | Set iff `target_kind = lore_entry`. `ON DELETE SET NULL` (not the Postgres-default `RESTRICT`) is required so deleting the target entry never blocks the delete (FR-020) |
| `target_actor_id` | `UUID` FK → `world_actors.id` `ON DELETE SET NULL`, nullable | Set iff `target_kind = actor`. Same `ON DELETE SET NULL` rationale as above |
| `created_at` | `TIMESTAMPTZ` | Recomputed (old rows for this source entry deleted, new ones inserted) on every save |

**Validation rules**: Exactly one of `target_lore_entry_id`/`target_actor_id` is non-null **at insert time**, matching `target_kind` (DB check constraint, enforced on write; the `ON DELETE SET NULL` above can null out the FK afterward without violating this constraint, since the constraint isn't re-checked on that later `UPDATE`). On every save of `source_lore_entry_id`'s content, the full set of `[[...]]` occurrences is re-extracted and this entry's outgoing rows are replaced wholesale (delete-then-insert in the same transaction) — never incrementally diffed, keeping the "what a revision's links looked like at that time" question answerable by joining through the historical `world_lore_revisions.content_markdown` re-parse if ever needed, while the *current* backlink list is always this table's live state. A row whose `target_kind` says `lore_entry`/`actor` but whose corresponding FK column has since gone `NULL` (because the target was deleted) is treated as unresolved by every read path (queries/rendering), the same as a `target_kind = unresolved` row — no separate migration/backfill of `target_kind` itself is needed when a target is deleted.

**Note**: The "linked from" list shown on a target (lore entry or actor) is a query, not a stored column: `SELECT source_lore_entry_id FROM world_lore_links WHERE target_lore_entry_id = :id` (or `target_actor_id = :id`).

## `world_lore_revisions`

Immutable append-only history (FR-016/017/018, research.md §4).

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID` PK | |
| `lore_entry_id` | `UUID` FK → `world_lore_entries.id`, cascade delete | |
| `content_markdown` | `TEXT NOT NULL` | Full snapshot, not a diff |
| `author_id` | `UUID` FK → user | Who saved this revision |
| `restored_from_revision_id` | `UUID` FK → `world_lore_revisions.id`, nullable | Set iff this revision was created by a "restore" action (FR-018), for history-UI provenance |
| `created_at` | `TIMESTAMPTZ` | |

**Validation rules**: Never updated or deleted after insert (enforced by omitting any UPDATE/DELETE Diesel query against this table in application code; no DB-level immutability trigger is required at this scale, matching the "keep it simple" tone of research.md §4). `world_lore_entries.current_revision_id` always points at the most recently inserted row for that entry.

## `world_lore_image_assets`

One row per uploaded/pasted image (FR-008/009), mirroring the existing `canvas_image_assets` table's shape (spec 002) but scoped to lore entries instead of scenes.

| Column | Type | Notes |
|---|---|---|
| `id` | `UUID` PK | Also used as the RustFS object-key stem (`{id}.webp`, `{id}-thumb.webp`) — never the filename |
| `lore_entry_id` | `UUID` FK → `world_lore_entries.id`, cascade delete | |
| `uploaded_by` | `UUID` FK → user | |
| `original_filename` | `TEXT`, nullable | Stored only for the uploader's own reference (e.g. "what did I paste"), never used in any URL or storage key (FR-011) |
| `content_type` | `TEXT NOT NULL` | Source MIME type, pre-transcode |
| `byte_size` | `BIGINT NOT NULL` | Post-transcode size, enforced ≤ 25 MB (FR-010) before this row is committed |
| `created_at` | `TIMESTAMPTZ` | |

**Validation rules**: Row is only inserted after a successful RustFS write of both the full-size and thumbnail WebP objects (matches the existing `canvas_image_assets` pattern of "write object, then record row" — no row for a failed/partial upload, satisfying the edge case "entry's content is not left referencing a broken/missing asset").

## Reused, unmodified entities

- **`world_actors`** (spec 010): valid link target (`world_lore_links.target_actor_id`); no schema change to this table itself, only a new incoming FK relationship from `world_lore_links`.
- **`world_members`** / world role resolution (spec 009/010): supplies the assignable-subject pool for `world_lore_permissions` and the "DM = Owner or GM role" authorization check, via a generalized version of `auth/actor_permissions.rs::is_dm_of_world` (parameterized by entry type or duplicated verbatim into `auth/lore_permissions.rs` — an implementation-detail choice for tasks.md, not a data-model concern).

## Entity relationship summary

```text
World ──1:N── LoreEntry ──1:N── LoreRevision (current_revision_id points back to latest)
                  │
                  ├──1:N── LorePermission ──N:1── WorldMember
                  │
                  ├──1:N── LoreLink (source) ──N:1── LoreEntry (target, nullable)
                  │                          └──N:1── Actor (target, nullable)
                  │
                  └──1:N── LoreImageAsset
```
