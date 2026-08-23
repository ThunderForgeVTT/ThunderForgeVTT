---

description: "Task list for feature implementation"
---

# Tasks: World Lore Wiki

**Input**: Design documents from `/specs/012-lore-wiki/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md (all present)

**Tests**: Not explicitly requested in spec.md — no dedicated TDD phase is generated. Server-side resolver tests follow the existing inline `#[tokio::test]` convention (see `graphql/mutations_actors.rs`) and are folded into each mutation/query's implementation task rather than split into a separate contract-test phase.

**Organization**: Tasks are grouped by user story (spec.md priorities) to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1–US5)
- Every task includes an exact file path

## Path Conventions

Existing two-project split (plan.md "Project Structure"): `src/server/` (Rust/Axum/Diesel/async-graphql backend), `apps/web/` (React/TypeScript frontend). No new top-level project.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Add the new dependencies this feature needs; nothing here is user-story-specific.

- [X] T001 Add `comrak`, `ammonia`, and `slug` crate dependencies to `src/server/Cargo.toml` (research.md §1, §3)
- [X] T002 Run `cargo build` in `src/server` to confirm the new dependencies resolve cleanly against the existing dependency tree

**Checkpoint**: New crates compile; ready for schema/module work.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Database schema, permission model, Markdown/link/slug utilities, and GraphQL/routing scaffolding that every user story depends on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T003 Create Diesel migration `create_world_lore_entries` (`id`, `world_id`, `title`, `slug`, `content`, `current_revision_id` nullable, `created_by`, `created_at`, `updated_at`; unique `(world_id, slug)`) in `src/server/migrations/<ts>_create_world_lore_entries/{up,down}.sql` (data-model.md)
- [X] T004 Create Diesel migration `create_world_lore_revisions` (`id`, `lore_entry_id` FK cascade, `content_markdown`, `author_id`, `restored_from_revision_id` nullable self-FK, `created_at`) in `src/server/migrations/<ts>_create_world_lore_revisions/{up,down}.sql` (depends on T003 for the FK target)
- [X] T005 Create Diesel migration `create_world_lore_permissions` (`id`, `lore_entry_id` FK cascade, `world_member_user_id`, `permission_level` enum, `created_at`, `updated_at`; unique `(lore_entry_id, world_member_user_id)`) in `src/server/migrations/<ts>_create_world_lore_permissions/{up,down}.sql` (depends on T003)
- [X] T006 Create Diesel migration `create_world_lore_links` (`id`, `source_lore_entry_id` FK cascade, `raw_title`, `target_kind` enum, `target_lore_entry_id` nullable FK **`ON DELETE SET NULL`**, `target_actor_id` nullable FK to `world_actors` **`ON DELETE SET NULL`**, `created_at`; check constraint enforcing exactly one target set at insert time) in `src/server/migrations/<ts>_create_world_lore_links/{up,down}.sql` — `ON DELETE SET NULL` (not the Postgres-default `RESTRICT`) is required so deleting a linked-to entry/actor never blocks that delete (FR-020, data-model.md) (depends on T003)
- [X] T007 Create Diesel migration `create_world_lore_image_assets` (`id`, `lore_entry_id` FK cascade, `uploaded_by`, `original_filename` nullable, `content_type`, `byte_size`, `created_at`) in `src/server/migrations/<ts>_create_world_lore_image_assets/{up,down}.sql` (depends on T003)
- [X] T008 Run `diesel migration run` against the local dev DB and regenerate `src/server/src/schema.rs` with the five new tables (depends on T003–T007)
- [X] T009 [P] Add `LoreEntry`, `LoreRevision`, `LorePermission`, `LoreLink`, `LoreImageAsset` Diesel `Queryable`/`Insertable` structs to `src/server/src/models.rs` (depends on T008)
- [X] T010 Create `src/server/src/auth/lore_permissions.rs` generalizing `auth/actor_permissions.rs`: `is_dm_of_world` reuse, `effective_lore_permission(state, user_id, is_admin, lore_entry_id)`, `require_lore_permission(..., minimum: ActorPermissionLevel)` (depends on T009)
- [X] T011 Create `src/server/src/markdown/mod.rs` implementing GFM parse + sanitize-to-HTML via `comrak` (GFM extensions on, `unsafe_` off) piped through `ammonia` (research.md §1) (depends on T001)
- [X] T012 [P] Create `src/server/src/markdown/slug.rs` implementing title → urlified slug via the `slug` crate, with per-`world_id` collision disambiguation (numeric suffix) (research.md §3) (depends on T001)
- [X] T013 Create `src/server/src/markdown/links.rs` implementing `[[Title]]` / `[[Title|Display]]` extraction and resolution against `world_lore_entries.title` and `world_actors.name` scoped to a world (research.md §2) (depends on T008)
- [X] T014 Register the new `auth::lore_permissions` and `markdown::{mod, slug, links}` modules in `src/server/src/main.rs` (or the relevant `mod.rs` files) (depends on T010, T011, T012, T013)
- [X] T015 [P] Add `GraphQLLoreEntry`, `GraphQLLoreRevision`, `GraphQLLorePermission`, `GraphQLLoreImageAsset` GraphQL object types to `src/server/src/graphql/types.rs`, reusing the existing `ActorPermissionLevel` enum (contracts/lore-crud.md, lore-permissions.md, lore-revisions.md, lore-images.md) (depends on T009)
- [X] T016 [P] Add `CreateLoreEntryInput`, `UpdateLoreEntryInput`, `SetLorePermissionInput` GraphQL input types to `src/server/src/graphql/input_types.rs` (contracts/lore-crud.md, lore-permissions.md) (depends on T009)
- [X] T017 [P] Add `LoreEntryRecord`, `LoreRevisionRecord`, `LorePermissionRecord`, `LoreImageAssetRecord` TypeScript types to `apps/web/src/types/lore.ts` (contracts/*.md)
- [X] T018 [P] Create `apps/web/src/api/lore.ts` with `fetch`-based GraphQL call stubs mirroring `apps/web/src/api/actors.ts`'s `postGraphQL`/CSRF pattern (depends on T017)
- [X] T019 Add frontend routes `/world/:id/lore/:slug/view`, `/world/:id/lore/:slug/edit`, `/world/:id/lore/:slug/history` to `apps/web/src/routes/AppRoutes.tsx` and lazy-loader entries in `apps/web/src/routes/pageLoaders.ts` (mirrors the existing `/world/:id/actor/:actorId/view|edit` routes)

**Checkpoint**: Schema, permission model, Markdown/link/slug utilities, GraphQL scaffolding, and frontend routing shell all exist — user story implementation can begin.

---

## Phase 3: User Story 1 - DM authors a lore entry with rich Markdown (Priority: P1) 🎯 MVP

**Goal**: A DM can create a lore entry, write full GFM Markdown content, save it, and see it correctly rendered; entries appear in a world-scoped lore index.

**Independent Test**: As a DM, create a lore entry containing a table, task list, code block, blockquote, headings, and a link; save; confirm the rendered view matches GFM rendering (quickstart.md US1).

### Implementation for User Story 1

- [X] T020 [US1] Implement `createLoreEntry` mutation in `src/server/src/graphql/mutations_lore.rs`: DM-only check via `auth::lore_permissions::is_dm_of_world`, insert `world_lore_entries` row, generate initial slug via `markdown::slug`, render initial content via `markdown::mod` if provided, insert first `world_lore_revisions` row (contracts/lore-crud.md) (depends on T010, T011, T012, T015, T016)
- [X] T021 [US1] Implement `updateLoreEntry` mutation in `src/server/src/graphql/mutations_lore.rs`: Editor/Owner permission check, enforce 25 MB content cap (FR-010a) before any write; when `content` is provided, require `expectedCurrentRevisionId` and compare it against the entry's live `current_revision_id` inside the same transaction as the insert — reject outright with a conflict error on mismatch (FR-019, contracts/lore-crud.md), never overwrite; on match, append a new `world_lore_revisions` row, update `current_revision_id`, re-render `renderedHtml` (depends on T020)
- [X] T022 [US1] Implement `deleteLoreEntry` mutation in `src/server/src/graphql/mutations_lore.rs`: Owner-level permission check (FR-021), cascade delete per data-model.md, does not block on other entries' dangling `world_lore_links` (FR-020) (depends on T020)
- [X] T023 [P] [US1] Implement `worldLoreEntries(worldId)` and `loreEntry(worldId, slug)` queries in `src/server/src/graphql/queries/lore.rs`: both reuse the existing `require_world_member`/`require_visible_world` check (same as every other world-scoped query) to reject non-members before returning any rows; among members, listing is unfiltered by per-entry permission (permission surfaced via `myPermissionLevel`), and `loreEntry` returns `null` for a stale/missing slug (contracts/lore-crud.md) (depends on T010, T015)
- [X] T024 [US1] Wire `mutations_lore` and `queries::lore` into the GraphQL schema root in `src/server/src/graphql/mod.rs` (or wherever the schema is assembled) (depends on T020, T021, T022, T023)
- [X] T025 [P] [US1] Create `apps/web/src/pages/world/compendium/LoreCompendiumTab.tsx` (lore index list + "new entry" control for DM, mirrors `NpcCompendiumTab.tsx`) (depends on T018)
- [X] T026 [US1] Replace the "Lore" `ComingSoonTab` slot (if present) or add a new "Lore" tab in `apps/web/src/pages/world/compendium/WorldCompendiumPage.tsx` wired to `LoreCompendiumTab` (depends on T025)
- [X] T027 [P] [US1] Create `apps/web/src/pages/world/lore/LoreMarkdownRenderer.tsx` rendering the server-provided sanitized `renderedHtml` (with a client-side code-block syntax-highlight pass) (depends on T018)
- [X] T028 [P] [US1] Create `apps/web/src/pages/world/lore/LoreMarkdownEditor.tsx` as a plain `<textarea>`-based editor with save/cancel (paste-image and `[[`-autocomplete added in US2/US3) (depends on T018)
- [X] T029 [US1] Create `apps/web/src/pages/world/lore/LoreEntryDetailPage.tsx` (view/edit modes, mirrors `ActorDetailPage.tsx`) wiring `LoreMarkdownRenderer`/`LoreMarkdownEditor` to the `loreEntry`/`updateLoreEntry`/`deleteLoreEntry` API calls (depends on T027, T028, T019)

- [X] T029a [US1] Implement `setLorePermission` mutation and `loreEntryPermissions(loreEntryId)` query in `src/server/src/graphql/mutations_lore_permissions.rs`: DM-only check via `auth::lore_permissions::is_dm_of_world` for both (mirrors `mutations_actor_permissions.rs` exactly — no other permission level, including entry-level Owner, may open or change the block); `setLorePermission` upserts by `(lore_entry_id, world_member_user_id)`, bumping `updated_at` on an existing row; `loreEntryPermissions` returns every world member with their explicit level or an indication of the implicit default Viewer (contracts/lore-permissions.md, FR-003) (depends on T010, T015, T016)
- [X] T029b [US1] Wire `mutations_lore_permissions` into the GraphQL schema root in `src/server/src/graphql/mod.rs` (depends on T029a, T024)
- [X] T029c [US1] Create `apps/web/src/pages/world/lore/LoreOwnershipBlock.tsx` (mirrors `ActorOwnershipBlock.tsx`: lists every world member + DM with their level, DM-only edit controls) wired into `LoreEntryDetailPage.tsx`, visible only when the viewer is DM (depends on T029a, T018, T029)

**Checkpoint**: User Story 1 is fully functional and independently testable — DM can author, save, view, delete, and delegate ownership of a rich-Markdown lore entry.

---

## Phase 4: User Story 2 - Authors correlate lore entries with each other and with actors via in-text links (Priority: P1)

**Goal**: `[[Title]]` links in an entry's body resolve to another lore entry or an actor, and the target shows an automatic "linked from" backlink list.

**Independent Test**: From Entry A, link to Entry B and to an actor; confirm both links render correctly and both targets list Entry A under "linked from" (quickstart.md US2).

### Implementation for User Story 2

- [X] T030 [US2] Extend `updateLoreEntry` (and `createLoreEntry`'s initial-content path) in `src/server/src/graphql/mutations_lore.rs` to call `markdown::links::extract_and_resolve`, replacing this entry's `world_lore_links` source rows transactionally with the freshly resolved set (research.md §2) (depends on T013, T020, T021)
- [X] T031 [P] [US2] Add a `linkedFrom: [GraphQLLoreEntry!]!` field resolver on `GraphQLLoreEntry` in `src/server/src/graphql/types.rs`, querying `world_lore_links WHERE target_lore_entry_id = :id` (depends on T015, T030)
- [X] T032 [P] [US2] Add a `loreLinkedFrom: [GraphQLLoreEntry!]!` field resolver on the existing Actor GraphQL type in `src/server/src/graphql/queries/actor.rs`, querying `world_lore_links WHERE target_actor_id = :id` (depends on T030)
- [X] T033 [P] [US2] Add a lore/actor title-prefix search query (e.g. `loreLinkTargets(worldId, prefix)`) in `src/server/src/graphql/queries/lore.rs` for the editor's `[[`-autocomplete, returning distinct disambiguated lore-entry and actor candidates (FR-007a) (depends on T010)
- [X] T034 [US2] Extend `LoreMarkdownEditor.tsx` in `apps/web/src/pages/world/lore/LoreMarkdownEditor.tsx` with a `[[`-trigger autocomplete popover (using existing `@radix-ui/react-popover`) calling `loreLinkTargets`, inserting the resolved `[[Title]]` text at the cursor (depends on T028, T033)
- [X] T035 [US2] Extend `LoreMarkdownRenderer.tsx` in `apps/web/src/pages/world/lore/LoreMarkdownRenderer.tsx` to style unresolved/broken `[[...]]` links distinctly (server marks them in `renderedHtml`, per research.md §2) (depends on T027, T030)
- [X] T036 [US2] Add a "Linked from" list section to `apps/web/src/pages/world/lore/LoreEntryDetailPage.tsx` using the new `linkedFrom` field (depends on T029, T031)
- [X] T037 [US2] Add a "Linked from (lore)" list section to `apps/web/src/pages/world/actor/ActorDetailPage.tsx` using the new `loreLinkedFrom` field (depends on T032)

**Checkpoint**: User Stories 1 and 2 both work independently — entries correlate with each other and with actors, with live backlinks.

---

## Phase 5: User Story 3 - Paste and manage images inline (Priority: P2)

**Goal**: Pasting/dropping an image into the editor uploads, processes, and inserts it automatically; oversized/unsupported files are rejected with a clear error.

**Independent Test**: Paste a clipboard image into the editor; confirm it uploads, renders inline within ~10s, and survives a reload (quickstart.md US3).

### Implementation for User Story 3

- [X] T038 [US3] Extend `transcode_to_webp` in `src/server/src/storage/transcode.rs` to also produce a normalized full-size rendition (max 2048px longest edge) and a 256px thumbnail, both WebP, using the existing `image` crate's resize (research.md §5) (depends on T001)
- [X] T039 [US3] Implement `uploadLoreImage` mutation in `src/server/src/graphql/mutations_lore_images.rs`: Editor/Owner permission check, enforce 25 MB cap pre-decode (FR-010), call the extended transcode pipeline and surface any undecodable/unsupported-format error from the `image` crate as a clean GraphQL error (not a panic), write both objects via existing `storage/rustfs.rs::write_object` (per-object STS-scoped, ADR-039), insert `world_lore_image_assets` row only after both writes succeed (contracts/lore-images.md) (depends on T010, T038, T007)
- [X] T040 [US3] Wire `mutations_lore_images` into the GraphQL schema root alongside `mutations_lore` in `src/server/src/graphql/mod.rs` (depends on T039)
- [X] T041 [US3] Extend `LoreMarkdownEditor.tsx` in `apps/web/src/pages/world/lore/LoreMarkdownEditor.tsx` with `paste`/`drop` handlers that intercept image `DataTransfer` items, call `uploadLoreImage`, and insert `![](url)` at the cursor on success; surface a clear error toast on rejection (depends on T028, T039)

**Checkpoint**: User Stories 1–3 all work independently — entries support rich inline images alongside text and correlation.

---

## Phase 6: User Story 4 - Share a lore entry via a readable, human-friendly URL (Priority: P2)

**Goal**: Every lore entry is reachable at a human-readable, urlified, collision-disambiguated URL that updates when the title changes and is denied to non-Viewers.

**Independent Test**: View a lore entry's URL, confirm it contains a readable slug; rename the entry, confirm the URL updates and the entry stays reachable (quickstart.md US4).

### Implementation for User Story 4

- [X] T042 [US4] Extend `updateLoreEntry` in `src/server/src/graphql/mutations_lore.rs` to regenerate `slug` (via `markdown::slug`, with collision disambiguation) whenever `title` changes, keeping `id` stable (FR-014) (depends on T012, T021)
- [X] T043 [US4] Enforce Viewer-or-above access denial in the `loreEntry(worldId, slug)` query in `src/server/src/graphql/queries/lore.rs` for non-members (FR-015) (depends on T023)
- [X] T044 [P] [US4] Add a "copy link" control to `apps/web/src/pages/world/lore/LoreEntryDetailPage.tsx` that copies the current `/world/:id/lore/:slug` URL (depends on T029)
- [X] T045 [P] [US4] Add a not-found/graceful state to `LoreEntryDetailPage.tsx` for a `null` `loreEntry` response (stale slug or denied access) (depends on T029, T043)

**Checkpoint**: User Stories 1–4 all work independently — entries have durable, shareable, human-readable URLs.

---

## Phase 7: User Story 5 - View and restore prior revisions (Priority: P3)

**Goal**: A DM (or any Editor/Owner) can view an entry's full revision history and restore it to any prior revision without losing history.

**Independent Test**: Save an entry three times, open its history, confirm all revisions listed, restore an earlier one, confirm content matches and history is preserved (quickstart.md US5).

### Implementation for User Story 5

- [X] T046 [US5] Implement `loreEntryRevisions(loreEntryId)` query in `src/server/src/graphql/queries/lore.rs`: Viewer-or-above check, newest-first ordering, re-rendering each historical `content_markdown` via `markdown::mod` for `renderedHtml` (contracts/lore-revisions.md) (depends on T010, T011, T015)
- [X] T047 [US5] Implement `restoreLoreRevision(revisionId)` mutation in `src/server/src/graphql/mutations_lore.rs`: Editor/Owner check on the revision's parent entry, appends a new `world_lore_revisions` row with `restored_from_revision_id` set, updates the entry's `content`/`current_revision_id`, re-extracts links via `markdown::links` (FR-018) (depends on T030, T046)
- [X] T048 [US5] Wire `loreEntryRevisions`/`restoreLoreRevision` into the GraphQL schema root in `src/server/src/graphql/mod.rs` (depends on T046, T047)
- [X] T049 [US5] Create `apps/web/src/pages/world/lore/LoreRevisionHistory.tsx` (chronological revision list, single-revision viewer, restore action) wired to the `/world/:id/lore/:slug/history` route (depends on T019, T046, T047)
- [X] T050 [US5] Add a "View history" link from `apps/web/src/pages/world/lore/LoreEntryDetailPage.tsx` to the history route (depends on T029, T049)

**Checkpoint**: All five user stories are independently functional.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Verification and final wiring that spans multiple user stories.

- [ ] T051 [P] Run `cargo check` and `cargo test` in `src/server` to confirm the new resolvers and inline `#[tokio::test]` coverage pass (constitution Principle V)
- [ ] T052 [P] Run `pnpm --filter @thunderforge/web build` and `pnpm --filter @thunderforge/web lint` in `apps/web`
- [ ] T053 Execute every scenario in `specs/012-lore-wiki/quickstart.md` against a running local dev stack (`docker compose up`), including the cross-cutting deletion/ownership-block/upload-size checks
- [X] T054 [P] Confirm the world-removal cascade deletes a departed member's `world_lore_permissions` rows (mirrors the existing actor-permission cascade, spec 010) by exercising it against `src/server/src/auth/lore_permissions.rs` and the relevant world-membership-removal path

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately.
- **Foundational (Phase 2)**: Depends on Setup completion — BLOCKS all user stories.
- **User Stories (Phase 3–7)**: All depend on Foundational phase completion.
  - US1 has no dependency on any other story.
  - US2 depends on US1's entry CRUD existing (extends `createLoreEntry`/`updateLoreEntry`) but is otherwise independently testable once US1 ships.
  - US3 depends only on Foundational + US1's editor shell (`LoreMarkdownEditor.tsx` from T028); independent of US2.
  - US4 depends only on Foundational + US1's entry/query plumbing; independent of US2/US3.
  - US5 depends on Foundational + US1's revision-row creation (T021) and US2's link re-extraction helper (T030, reused by restore).
- **Polish (Phase 8)**: Depends on all desired user stories being complete.

### Parallel Opportunities

- T009, T012 (Phase 2) can run in parallel with each other once T008/T001 land.
- T015, T016, T017 (Phase 2) can run in parallel once T009 lands.
- T023, T025, T027, T028 (Phase 3) can each run in parallel once their individual dependencies land (different files).
- T029a (Phase 3) can run in parallel with T023/T025/T027/T028 once T010/T015/T016 land (different files); T029c depends on T029a + T029.
- T031, T032, T033 (Phase 4) can run in parallel once T030 lands.
- T044, T045 (Phase 6) can run in parallel.
- T051, T052, T054 (Phase 8) can run in parallel.

---

## Parallel Example: User Story 1

```bash
# Once T020/T021/T022 (mutations) and T023 (queries) are wired via T024:
Task: "Create apps/web/src/pages/world/compendium/LoreCompendiumTab.tsx"
Task: "Create apps/web/src/pages/world/lore/LoreMarkdownRenderer.tsx"
Task: "Create apps/web/src/pages/world/lore/LoreMarkdownEditor.tsx"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (blocks everything)
3. Complete Phase 3: User Story 1 — DM can author, save, view, and delete a rich-Markdown lore entry
4. **STOP and VALIDATE**: run quickstart.md's US1 section independently
5. Demo if ready — this alone delivers "a wiki inside our app" without correlation/images/sharing/history yet

### Incremental Delivery

1. Setup + Foundational → foundation ready
2. US1 → validate → demo (MVP)
3. US2 (correlation) → validate → demo — delivers the "build and correlate" value explicitly named in the original request
4. US3 (images) → validate → demo
5. US4 (shareable URLs) → validate → demo
6. US5 (revision history / "micro repo" behavior) → validate → demo
7. Polish

### Suggested Task Ordering for a Single Implementer

Sequential by phase (T001→T054) is safe and matches dependency order above; within Phase 2 and within each story phase, [P]-marked tasks may be reordered or batched freely.
