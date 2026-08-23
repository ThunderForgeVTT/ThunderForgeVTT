---

description: "Task list for feature implementation"
---

# Tasks: Items & Inventory System

**Input**: Design documents from `/specs/013-items-inventory/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md (all present)

**Tests**: Not explicitly requested in spec.md — no dedicated TDD phase is generated. Server-side resolver tests follow the existing inline `#[tokio::test]` convention (see `graphql/mutations_actors.rs`) and are folded into each mutation/query's implementation task rather than split into a separate contract-test phase. Browser-level verification for the read-only browsing story (US4) uses a Playwright e2e task, mirroring spec 011's precedent.

**Organization**: Tasks are grouped by user story (spec.md priorities) to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1–US5)
- Every task includes an exact file path

## Path Conventions

Existing two-project split (plan.md "Project Structure"): `src/server/` (Rust/Axum/Diesel/async-graphql backend), `apps/web/` (React/TypeScript frontend). No new top-level project.

**Note on spec 012 (Lore Wiki) dependency**: User Story 3 (lore in-text links) extends spec 012's `world_lore_links` table and `markdown/links.rs` module (contracts/item-lore-links.md). If spec 012 has not yet been implemented when this feature is worked, defer Phase 5 (US3) until it has — every other phase (US1, US2, US4, US5) has no dependency on spec 012 and can ship independently, per plan.md's Structure Decision.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Enable the one new piece of infrastructure this feature needs (Postgres trigram similarity); nothing here is user-story-specific.

- [X] T001 Create Diesel migration `enable_pg_trgm` (`CREATE EXTENSION IF NOT EXISTS pg_trgm;` / `DROP EXTENSION IF EXISTS pg_trgm;`) in `src/server/migrations/<ts>_enable_pg_trgm/{up,down}.sql` (research.md §3)

**Checkpoint**: `pg_trgm` available for the trigram index created in Phase 2; ready for schema work.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Database schema, permission model, and GraphQL/routing scaffolding that every user story depends on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T002 Create Diesel migration `create_world_items` (`id`, `world_id` FK cascade, `name`, `description` nullable, `icon_asset_id` nullable, `created_by`, `created_at`, `updated_at`; trigram GIN index on `name` via `pg_trgm`, e.g. `CREATE INDEX world_items_name_trgm_idx ON world_items USING GIN (name gin_trgm_ops);`) in `src/server/migrations/<ts>_create_world_items/{up,down}.sql` (data-model.md, depends on T001)
- [X] T003 Create Diesel migration `create_world_item_permissions` (`id`, `item_id` FK cascade, `world_member_user_id`, `permission_level` enum, `created_at`, `updated_at`; unique `(item_id, world_member_user_id)`) in `src/server/migrations/<ts>_create_world_item_permissions/{up,down}.sql` (depends on T002)
- [X] T004 Create Diesel migration `create_world_item_effects` (`id`, `item_id` FK cascade, `effect_type` enum `heal`/`damage`/`modifier`/`attack_roll`, `formula`, `target`, `trigger_kind` enum `on_use`/`passive` nullable, `sort_order` int default 0, `created_at`, `updated_at`) in `src/server/migrations/<ts>_create_world_item_effects/{up,down}.sql` (data-model.md, depends on T002)
- [X] T005 Create Diesel migration `create_world_item_shares` (`id`, `item_id` FK cascade, `share_code` varchar(32), `created_by`, `revoked` bool default false, `created_at`, `updated_at`) in `src/server/migrations/<ts>_create_world_item_shares/{up,down}.sql` (depends on T002)
- [X] T006 Create Diesel migration `create_world_actor_inventory` (`id`, `actor_id` FK to `world_actors` cascade delete, `item_id` nullable FK to `world_items` **`ON DELETE SET NULL`**, `item_name_snapshot` text not null, `quantity` int not null check >= 0, `created_at`, `updated_at`; unique `(actor_id, item_id)`) in `src/server/migrations/<ts>_create_world_actor_inventory/{up,down}.sql` — `ON DELETE SET NULL` (not `RESTRICT`) so deleting an Item never blocks on outstanding inventory rows (FR-017, data-model.md) (depends on T002)
- [X] T007 Run `diesel migration run` against the local dev DB and regenerate `src/server/src/schema.rs` with the five new tables (depends on T002–T006)
- [X] T008 [P] Add `WorldItem`, `ItemEffect`, `ItemShare`, `ItemPermission`, `ActorInventoryEntry` Diesel `Queryable`/`Insertable` structs to `src/server/src/models.rs` (depends on T007)
- [X] T009 Create `src/server/src/auth/item_permissions.rs` generalizing `auth/actor_permissions.rs`: `is_dm_of_world` reuse, `effective_item_permission(state, user_id, is_admin, item_id)`, `require_item_permission(..., minimum: ActorPermissionLevel)` (mirrors spec 012's `auth/lore_permissions.rs`, reuses `ActorPermissionLevel` — no new enum) (depends on T008)
- [X] T010 [P] Add `GraphQLItem`, `GraphQLItemEffect`, `ItemEffectType`, `ItemEffectTrigger`, `GraphQLItemShareLink`, `SharedItemPreview`, `GraphQLInventoryEntry` GraphQL types to `src/server/src/graphql/types.rs`, reusing the existing `ActorPermissionLevel` enum for `myPermissionLevel` (contracts/graphql-items.md, graphql-inventory.md, item-share.md) (depends on T008)
- [X] T011 [P] Add `CreateItemInput`, `UpdateItemInput`, `ItemEffectInput`, `AddItemToInventoryInput`, `AdjustInventoryQuantityInput`, `CopySharedItemInput` GraphQL input types to `src/server/src/graphql/input_types.rs` (contracts/*.md) (depends on T008)
- [X] T012 [P] Add `WorldItemRecord`, `ItemEffectRecord`, `InventoryEntryRecord` TypeScript types to `apps/web/src/types/item.ts` and `apps/web/src/types/inventory.ts`; add `ItemShareLinkRecord`/`SharedItemPreviewRecord` to `apps/web/src/types/itemShare.ts` (contracts/*.md)
- [X] T013 [P] Create `apps/web/src/api/items.ts`, `apps/web/src/api/inventory.ts`, and `apps/web/src/api/itemShares.ts` with `fetch`-based GraphQL call stubs mirroring `apps/web/src/api/actors.ts`/`apps/web/src/api/actorShares.ts`'s `postGraphQL`/CSRF pattern (depends on T012)
- [X] T014 Add frontend routes `/world/:id/item/:itemId/view`, `/world/:id/item/:itemId/edit`, and `/item-share/:shareCode` to `apps/web/src/routes/AppRoutes.tsx` and lazy-loader entries in `apps/web/src/routes/pageLoaders.ts` (mirrors the existing `/world/:id/actor/:actorId/view|edit` and `/actor-share/:shareCode` routes)

**Checkpoint**: Schema, permission model, GraphQL scaffolding, and frontend routing shell all exist — user story implementation can begin.

---

## Phase 3: User Story 1 - DM authors an Item with a description and structured effects (Priority: P1) 🎯 MVP

**Goal**: A DM can create an Item with a name, optional description/icon, and structured effects (heal/damage/modifier/attack-roll), which appears in the world's Item catalog and Compendium tab.

**Independent Test**: As a DM, create an Item with a heal effect and a second Item with paired attack-roll + damage effects; save both; confirm they appear in the Item catalog with effects intact on reload (quickstart.md US1).

### Implementation for User Story 1

- [X] T015 [US1] Implement `createItem` mutation in `src/server/src/graphql/mutations_items.rs`: DM-only check via `auth::item_permissions::is_dm_of_world`, insert `world_items` row (`description`/`icon_asset_id` optional per Clarifications), no name-uniqueness check (FR-019, contracts/graphql-items.md) (depends on T009, T010, T011)
- [X] T016 [US1] Implement `updateItem` mutation in `src/server/src/graphql/mutations_items.rs`: Editor/Owner permission check via `require_item_permission` (depends on T015)
- [X] T017 [US1] Implement `deleteItem` mutation in `src/server/src/graphql/mutations_items.rs`: Owner-level permission check, cascade-deletes `world_item_permissions`/`world_item_effects`/`world_item_shares`, does NOT block on `world_actor_inventory` rows (nulled via `ON DELETE SET NULL`, T006) or (once spec 012 exists) `world_lore_links` rows referencing it (FR-017) (depends on T015)
- [X] T018 [US1] Implement `addItemEffect`/`updateItemEffect`/`removeItemEffect` mutations in `src/server/src/graphql/mutations_items.rs`: Editor/Owner check on the parent item, structural formula validation (non-empty, matches a minimal dice-grammar pattern per data-model.md) rejecting invalid formulas with a clear error before any write (FR-006, contracts/graphql-items.md) (depends on T015)
- [X] T019 [P] [US1] Implement `worldItems(worldId, search)` and `item(itemId)` queries in `src/server/src/graphql/queries/item.rs`: both reuse the existing `require_world_member`/`require_visible_world` check to reject non-members; `search` filters `name`/`description` instant-as-you-type (mirrors NPC catalog search) (contracts/graphql-items.md) (depends on T009, T010)
- [X] T020 [P] [US1] Implement `suggestItemName(worldId, name)` query in `src/server/src/graphql/queries/item.rs` using `similarity(name, :name) > 0.4 ORDER BY similarity DESC LIMIT 5` against the `pg_trgm` index (research.md §3, contracts/graphql-items.md) (depends on T002, T009)
- [X] T021 [US1] Wire `mutations_items` and `queries::item` into the GraphQL schema root in `src/server/src/graphql/mod.rs` (depends on T015–T020)
- [X] T022 [US1] Implement `setItemPermission` mutation and `itemPermissions(itemId)` query in `src/server/src/graphql/mutations_item_permissions.rs`: DM-only for both (mirrors `mutations_actor_permissions.rs`/spec 012's `mutations_lore_permissions.rs` exactly), upserts by `(item_id, world_member_user_id)` (FR-003) (depends on T009, T010, T011)
- [X] T023 [US1] Wire `mutations_item_permissions` into the GraphQL schema root (depends on T022, T021)
- [X] T024 [P] [US1] Create `apps/web/src/pages/world/compendium/ItemCompendiumTab.tsx` (searchable item catalog + DM-only "Add Item" control, mirrors `NpcCompendiumTab.tsx`) (depends on T013)
- [X] T025 [P] [US1] Create `apps/web/src/pages/world/compendium/ItemPreviewPanel.tsx` (right-docked preview: name, description, icon, effects; View always available, Edit gated on Editor/Owner; mirrors `ActorPreviewPanel.tsx`) (depends on T013)
- [X] T026 [US1] Replace `<ComingSoonTab label="Items" />` in `apps/web/src/pages/world/compendium/WorldCompendiumPage.tsx` (line ~75) with the `ItemCompendiumTab` + `ItemPreviewPanel` split view, matching the `npcs` tab's existing `grid gap-4 lg:grid-cols-[2fr_1fr]` composition (depends on T024, T025)
- [X] T027 [P] [US1] Create `apps/web/src/pages/world/item/ItemEffectEditor.tsx` (add/edit/remove structured effect rows: type select, formula text input, target text input, optional trigger-kind select) (depends on T013)
- [X] T028 [US1] Create `apps/web/src/pages/world/item/ItemDetailPage.tsx` (view/edit modes, mirrors `ActorDetailPage.tsx`) wiring `ItemEffectEditor` to `createItem`/`updateItem`/`deleteItem`/`addItemEffect`/`updateItemEffect`/`removeItemEffect` (depends on T027, T014)
- [X] T029 [P] [US1] Create `apps/web/src/pages/world/item/ItemOwnershipBlock.tsx` (mirrors `ActorOwnershipBlock.tsx`: lists every world member + DM with their level, DM-only edit controls) wired into `ItemDetailPage.tsx`, visible only when the viewer is DM (depends on T022, T013, T028)
- [X] T030 [US1] Wire the "did you mean?" hint into the Item-creation form (`ItemCompendiumTab.tsx`'s "Add Item" flow): debounce `suggestItemName` as the DM types a new name, render a non-blocking inline suggestion, never block save (FR-020) (depends on T020, T024)

**Checkpoint**: User Story 1 is fully functional and independently testable — a DM can author, save, view, delete, and delegate ownership of an Item with structured effects, browsable in the Compendium.

---

## Phase 4: User Story 2 - Actors hold Items in a quantity-based inventory (Priority: P1)

**Goal**: A user with Editor/Owner access to an Actor can add Items to that Actor's inventory with a quantity, adjust quantities, and remove entries; permission is governed by the Actor's own ownership block, not the Item's.

**Independent Test**: Add an Item to an Actor's inventory with quantity 3; confirm the sheet shows quantity 3; decrease to 2; remove the entry entirely (quickstart.md US2).

### Implementation for User Story 2

- [X] T031 [US2] Implement `addItemToInventory` mutation in `src/server/src/graphql/mutations_inventory.rs`: Editor/Owner check on `actorId` via the existing `auth::actor_permissions::require_actor_permission` (NOT `item_permissions` — inventory is Actor-scoped, FR-013/Assumptions), upsert via `ON CONFLICT (actor_id, item_id) DO UPDATE SET quantity = world_actor_inventory.quantity + excluded.quantity`, refreshing `item_name_snapshot` from the Item's current name (research.md §2, contracts/graphql-inventory.md) (depends on T006, T009)
- [X] T032 [US2] Implement `adjustInventoryQuantity` mutation in `src/server/src/graphql/mutations_inventory.rs`: Editor/Owner check on the entry's `actor_id`, sets `quantity` to the given absolute value, deletes the row and returns `null` when the result is 0 (FR-011) (depends on T031)
- [X] T033 [US2] Implement `removeInventoryEntry` mutation in `src/server/src/graphql/mutations_inventory.rs`: Editor/Owner check on the entry's `actor_id`, deletes the row outright (depends on T031)
- [X] T034 [P] [US2] Implement `actorInventory(actorId)` query in `src/server/src/graphql/queries/inventory.rs`: Viewer-or-above check on `actorId`, returns every row including deleted-item rows rendered via `item_name_snapshot` (contracts/graphql-inventory.md) (depends on T006)
- [X] T035 [US2] Wire `mutations_inventory` and `queries::inventory` into the GraphQL schema root in `src/server/src/graphql/mod.rs` (depends on T031–T034)
- [X] T036 [P] [US2] Create `apps/web/src/pages/world/actor/ActorInventoryPanel.tsx`: Item + quantity list, add/adjust/remove controls visible only to Editor/Owner on the Actor, deleted-item rows shown distinctly via `item_name_snapshot` (depends on T013)
- [X] T037 [US2] Wire `ActorInventoryPanel` into the existing `apps/web/src/pages/world/actor/ActorDetailPage.tsx` (depends on T036)

**Checkpoint**: User Stories 1 and 2 both work independently — Items exist and Actors can hold them with quantities, permissioned via the Actor's own ownership block.

---

## Phase 5: User Story 3 - Items are referenced from Lore the same way Actors are (Priority: P2)

**Goal**: `[[Item Name]]` in a lore entry's body resolves to an Item, alongside existing lore-entry/actor resolution; the Item gains a "linked from (lore)" backlink list.

**Independent Test**: From a lore entry, link to an Item; confirm the rendered link navigates to the Item and the Item lists that entry under "linked from"; delete the Item and confirm the link degrades to broken rather than blocking the delete (quickstart.md US3).

**Prerequisite**: Requires spec 012 (Lore Wiki)'s `world_lore_links` table and `markdown/links.rs` module to exist — see this file's top-level "Note on spec 012 dependency."

### Implementation for User Story 3

- [ ] T038 [US3] Create Diesel migration extending `world_lore_links`: add nullable `target_item_id UUID FK → world_items.id ON DELETE SET NULL` column and an `item` variant to the `target_kind` enum, extending the existing "exactly one target FK set at insert time" check constraint to cover it (contracts/item-lore-links.md) in `src/server/migrations/<ts>_add_item_target_to_world_lore_links/{up,down}.sql` (depends on T002, and on spec 012's `create_world_lore_links` migration already having run)
- [ ] T039 [US3] Extend `src/server/src/markdown/links.rs`'s `[[Title]]` extraction/resolution pass to additionally resolve against `world_items.name` scoped to the current world, presenting every kind of match (lore entry, actor, item) as distinct disambiguated candidates when a title matches more than one (FR-016, contracts/item-lore-links.md) (depends on T038)
- [ ] T040 [P] [US3] Add a `linkedFromLore: [GraphQLLoreEntry!]!` field resolver on `GraphQLItem` in `src/server/src/graphql/types.rs`, querying `world_lore_links WHERE target_item_id = :id` (depends on T038, T010)
- [ ] T041 [US3] Add a "Linked from (lore)" list section to `apps/web/src/pages/world/item/ItemDetailPage.tsx` using the new `linkedFromLore` field (depends on T028, T040)
- [ ] T042 [US3] Verify `deleteItem` (T017) leaves referencing `world_lore_links` rows with `target_item_id` nulled (via the migration's `ON DELETE SET NULL`) and that the existing broken-link render path (spec 012) treats them as unresolved with no new code path required — add a resolver test in `src/server/src/graphql/mutations_items.rs` confirming this (depends on T017, T038)

**Checkpoint**: User Stories 1–3 all work independently — Items correlate with lore entries exactly as actors do.

---

## Phase 6: User Story 4 - Any world member can browse the Item catalog read-only (Priority: P2)

**Goal**: A Player gets the identical browse/search/preview experience as the DM on the Compendium's Items tab, with create/edit affordances correctly absent based on their own permission level.

**Independent Test**: As a Player (non-DM) world member, open the Compendium's Items tab, confirm full browse/search/preview parity with the DM's view, confirm "Add Item" is absent, and confirm "Edit" only appears on Items where `myPermissionLevel` is Editor or Owner (quickstart.md US4).

### Implementation for User Story 4

- [X] T043 [US4] Add a Playwright e2e test to `apps/web/e2e/world-compendium-items.spec.ts`: as a Player world member, load the Compendium's Items tab, confirm the "Add Item" control is absent (verifies T024's DM-only gate), confirm search/select/preview work identically to the DM's experience, and confirm the preview panel omits "Edit" for an Item where the Player's `myPermissionLevel` is `VIEWER` (verifies T025's gate)

**Checkpoint**: User Stories 1, 2, and 4 all work independently — the same catalog/gating logic built in US1 correctly serves both roles with no story-specific implementation beyond this verification test.

---

## Phase 7: User Story 5 - Share an Item and copy it into another world (Priority: P3)

**Goal**: A member with Owner-level access to an Item can generate a share link; any logged-in viewer sees a read-only preview and can copy the Item (with cloned effects) into one of their own DM-level worlds as a fully independent record.

**Independent Test**: Generate a share link for an Item, open it as an unrelated user, confirm a read-only preview, copy it into a destination world, confirm a fully independent Item with cloned effects appears (quickstart.md US5).

### Implementation for User Story 5

- [X] T044 [US5] Implement `createItemShareLink`/`revokeItemShareLink` mutations in `src/server/src/graphql/mutations_item_shares.rs`, directly mirroring `mutations_actor_shares.rs`'s `generate_share_code`/create/revoke logic and its `created_by`-or-DM revoke rule (FR-022, FR-027, contracts/item-share.md) (depends on T009, T010)
- [X] T045 [US5] Implement `sharedItem(shareCode)` query in `src/server/src/graphql/mutations_item_shares.rs` (or a sibling query module): authenticated-only, no world-membership check, returns `SharedItemPreview` or a clear "not available" error for a revoked/missing link (mirrors `shared_actor_impl`) (depends on T044)
- [X] T046 [US5] Implement `copySharedItemToWorld` mutation in `src/server/src/graphql/mutations_item_shares.rs`: re-verifies DM-level access on `destinationWorldId` server-side, in one transaction inserts a new `world_items` row (fresh id, empty ownership block) and clones every `world_item_effects` row from the source (FR-025, FR-026, contracts/item-share.md) (depends on T044, T018)
- [X] T047 [US5] Confirm the existing `myDmWorlds` query (spec 010) is reused as-is for the "Copy to World" destination picker — no Item-specific variant needed (research.md §5); add a resolver test in `mutations_item_shares.rs` confirming it returns the caller's DM-level worlds unfiltered by content type (depends on T044)
- [X] T048 [US5] Wire `mutations_item_shares` (and its query) into the GraphQL schema root in `src/server/src/graphql/mod.rs` (depends on T044–T047)
- [X] T049 [P] [US5] Create `apps/web/src/pages/item-share/SharedItemPage.tsx`, mirroring `apps/web/src/pages/actor-share/SharedActorPage.tsx` (read-only preview + logged-in "Copy to World" picker) (depends on T013, T014)
- [X] T050 [P] [US5] Add "Share" (generate/revoke link) and "Copy to World" controls to `apps/web/src/pages/world/item/ItemDetailPage.tsx` and `SharedItemPage.tsx` respectively (depends on T028, T049, T044, T046)

**Checkpoint**: All five user stories are independently functional.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Verification and final wiring that spans multiple user stories.

- [X] T051 [P] Run `cargo check` and `cargo test` in `src/server` to confirm the new resolvers and inline `#[tokio::test]` coverage pass (constitution Principle V)
- [X] T052 [P] Run `pnpm --filter @thunderforge/web build` and `pnpm --filter @thunderforge/web lint` in `apps/web`
- [ ] T053 Execute every scenario in `specs/013-items-inventory/quickstart.md` against a running local dev stack (`docker compose up`), including the cross-cutting name-collision, optional-icon, deleted-item-inventory, and Actor-scoped-permission checks
- [X] T054 [P] Confirm the world-removal cascade deletes a departed member's `world_item_permissions` rows (mirrors the existing actor/lore-permission cascade) by exercising it against `src/server/src/auth/item_permissions.rs` and the relevant world-membership-removal path

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately.
- **Foundational (Phase 2)**: Depends on Setup completion — BLOCKS all user stories.
- **User Stories (Phase 3–7)**: All depend on Foundational phase completion.
  - US1 has no dependency on any other story.
  - US2 depends only on Foundational (its own table, `world_actor_inventory`, was created in Phase 2) and reuses the existing `world_actors` permission model — independent of US1's Item-authoring UI, though it needs at least one Item to exist to be exercised end-to-end.
  - US3 depends on Foundational + US1 (`world_items` must exist, T017's delete behavior is verified here) AND on spec 012 already being implemented (see top-of-file note) — independent of US2/US4/US5.
  - US4 depends on Foundational + US1's catalog/gating logic (T024/T025) — adds only a verification test, no new implementation.
  - US5 depends on Foundational + US1's `createItem`/`addItemEffect` (T017 delete semantics; T018 for effect cloning in T046) — independent of US2/US3/US4.
- **Polish (Phase 8)**: Depends on all desired user stories being complete.

### Parallel Opportunities

- T008, and after it lands, T010/T011/T012 (Phase 2) can run in parallel (different files).
- T019, T020 (Phase 3) can run in parallel once T009/T010 land.
- T024, T025, T027, T029 (Phase 3) can each run in parallel once their individual dependencies land (different files).
- T034 (Phase 4) can run in parallel with T031–T033 only after T031 lands (T032/T033 depend on T031's upsert existing first, since they mutate the same row shape — but T034, a read-only query, can be built in parallel with T031 once T006/T009 land).
- T040 (Phase 5) can run in parallel with T039 once T038 lands.
- T049, T050 (Phase 7) can run in parallel with T044–T048 once T013/T014/T028 land (frontend vs. backend files).
- T051, T052, T054 (Phase 8) can run in parallel.

---

## Parallel Example: User Story 1

```bash
# Once T015/T016/T017/T018 (mutations) and T019/T020 (queries) are wired via T021:
Task: "Create apps/web/src/pages/world/compendium/ItemCompendiumTab.tsx"
Task: "Create apps/web/src/pages/world/compendium/ItemPreviewPanel.tsx"
Task: "Create apps/web/src/pages/world/item/ItemEffectEditor.tsx"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (blocks everything)
3. Complete Phase 3: User Story 1 — DM can author, save, view, and delete an Item with structured effects, browsable in the Compendium
4. **STOP and VALIDATE**: run quickstart.md's US1 section independently
5. Demo if ready — this alone delivers the "description and configurable effects" value explicitly named in the original request

### Incremental Delivery

1. Setup + Foundational → foundation ready
2. US1 (Item authoring) → validate → demo (MVP)
3. US2 (Actor inventory) → validate → demo — delivers the "MMO-style inventory with quantity" value explicitly named in the original request
4. US3 (lore correlation) → validate → demo — requires spec 012 to exist first
5. US4 (read-only browsing) → validate → demo — thin verification pass on top of US1
6. US5 (share/copy across worlds) → validate → demo
7. Polish

### Suggested Task Ordering for a Single Implementer

Sequential by phase (T001→T054) is safe and matches dependency order above, EXCEPT Phase 5 (US3, T038–T042) should be deferred until spec 012 has shipped if it hasn't yet — skip straight from Phase 4 to Phase 6 (US4) and Phase 7 (US5), then return to Phase 5 once spec 012 lands. Within Phase 2 and within each story phase, [P]-marked tasks may be reordered or batched freely.
