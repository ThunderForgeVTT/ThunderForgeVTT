---

description: "Task list for World Abilities Compendium"
---

# Tasks: World Abilities Compendium

**Input**: Design documents from `/specs/025-world-abilities-compendium/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included. Every contract carries an explicit "Test expectations" section, research.md §6 records the testing decision, and the item precedent (spec 013) ships ~11 focused resolver tests. Note that **zero** frontend tests exist for items today — abilities deliberately do not inherit that gap.

**Organization**: Tasks are grouped by user story so each is independently implementable and testable.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1-US6)

## Path Conventions

Existing two-part layout: `src/server/` (Rust/Axum/async-graphql/Diesel) and `apps/web/` (React SPA). No `src/engine` changes — this feature has no canvas surface.

## Environment prerequisite

Every DB-backed `cargo test` needs `DATABASE_URL` plus running containers. A bare `cargo test` fails with `DATABASE_URL must be set` — an environment error, not a code failure:

```bash
docker compose up -d postgres rustfs
set -a && source .env && set +a
```

---

## Phase 1: Setup (Governance prerequisite + pre-existing bug fixes)

**Purpose**: Clear the one governance gate this feature depends on, plus two shipped bugs that affect **items today** and make correct four-kind link labelling impossible (research.md §3, defects 2-3).

- [ ] T001 Obtain acceptance of the DMCA guardrail determination in `docs/adrs/20260825-049-share_link_dmca_repository_determination.md` — the ADR is drafted (finding: share links are **not** a centralized public repository, conditional on six named invariants, covering actor/item shares retroactively). Record the accountable owner in its "Risk accepted" section and move Status from Proposed to Accepted. **Gates Phase 8 (US6) only**; US1-US5 do not depend on it
- [X] T002 [P] Widen `LoreLinkTargetKind` from `"LORE_ENTRY" | "ACTOR"` to include `"ITEM"` and `"ABILITY"` in `apps/web/src/types/lore.ts` — the `ITEM` variant was never added despite the backend returning it since spec 013
- [X] T003 [P] Replace the binary `detail:` ternary with a `Record<LoreLinkTargetKind, string>` label map in `apps/web/src/pages/world/lore/LoreMarkdownEditor.tsx` (~line 92) so item candidates stop displaying as "Actor"

**Checkpoint**: The guardrail is cleared (or US6 is knowingly dropped), and item link candidates label correctly in the `[[` autocomplete — both verifiable before any ability code exists.

**Follow-up worth doing, outside this feature's scope**: spec 015's Assumptions section states the platform "currently has no public compendium-sharing … feature" and that its guardrails are "preventative … not a retrofit of an existing one." That was factually wrong when written — actor sharing had already shipped. Correcting it would stop the same miss recurring.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The ability entity, its permission enforcement, and GraphQL registration. Every user story needs abilities to exist and to be permission-checked.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

**Scope note**: This phase delivers permission *enforcement* with defaults (DM ⇒ Owner, everyone else ⇒ Viewer). US5 later delivers the *management surface* for changing them. That split is why `world_ability_permissions` is created here rather than in US5 — `effective_ability_permission` cannot work without the table.

- [X] T004 Create migration `src/server/migrations/<ts>_create_world_abilities/{up,down}.sql` per data-model.md §1 — includes `updated_by` (an improvement over `world_items`, per FR-027), the `gm_only BOOLEAN NOT NULL DEFAULT FALSE` visibility column, and the `gin_trgm_ops` name index reusing the already-enabled `pg_trgm`
- [X] T005 Create migration `src/server/migrations/<ts>_create_world_ability_permissions/{up,down}.sql` per data-model.md §3 — app-supplied PK (no DB default), `UNIQUE (ability_id, user_id)` as the upsert conflict target
- [X] T006 Add `world_abilities` and `world_ability_permissions` `table!` blocks, `joinable!` lines, and `allow_tables_to_appear_in_same_query!` entries in `src/server/src/schema.rs`
- [X] T007 Add `WorldAbility`/`NewWorldAbility` and `AbilityPermission`/`NewAbilityPermission` structs in `src/server/src/models.rs` — field order MUST match `schema.rs`; `NewAbilityPermission` includes `id` (no DB default), `NewWorldAbility` omits it
- [X] T008 Create `src/server/src/auth/ability_permissions.rs` with `effective_ability_permission` and `require_ability_permission` per contracts/graphql-abilities.md, and register `pub mod ability_permissions;` in `src/server/src/auth/mod.rs` — reuse the existing `ActorPermissionLevel` enum, do NOT define a fourth copy
- [X] T009 Add `AbilityClassification` enum, `GraphQLAbility` (SimpleObject + `#[graphql(complex)]`, with a `gmOnly: Boolean!` field and `from_row`/`moderated_placeholder` constructors), and `GraphQLAbilityPermission` in `src/server/src/graphql/types.rs`
- [X] T010 Add `ModerationEntityType::WorldAbility ↔ "world_ability"` in `src/server/src/graphql/types.rs` and the corresponding owner/world lookup match arm in `src/server/src/graphql/mutations_moderation.rs`
- [X] T011 Create `src/server/src/graphql/queries/ability.rs` and `src/server/src/graphql/mutations_abilities.rs` as registered shells, wire `pub mod`/re-exports in `src/server/src/graphql/queries/mod.rs` and `src/server/src/graphql.rs`, and add `AbilityQuery`/`AbilityMutation` to the `QueryRoot`/`MutationRoot` merged objects in `src/server/src/graphql.rs`
- [X] T012 [P] Create `apps/web/src/types/ability.ts` with `AbilityClassification`, `WorldAbilityRecord`, and `AbilityPermissionRecord` per contracts/graphql-abilities.md
- [X] T013 [P] Add server tests in `src/server/src/auth/ability_permissions.rs` covering: no permission row ⇒ Viewer; DM ⇒ Owner regardless of rows; unparseable DB level string ⇒ Viewer; `require_ability_permission` rejects below-minimum with a `FORBIDDEN` code

**Checkpoint**: `cargo check -p thunderforge` clean, migrations apply, abilities are permission-checked. User story work can begin.

---

## Phase 3: User Story 1 - A GM authors an ability and it appears in the Compendium (Priority: P1) 🎯 MVP

**Goal**: Replace the Compendium's last "coming soon" placeholder with a real searchable ability table plus row-preview panel, with classifications labelled in the active system's own vocabulary.

**Independent Test**: Open `/world/:id/compendium?tab=abilities` as a GM, create an ability, confirm it appears without a reload, search for it, select its row, and confirm the preview panel shows what was entered. Delivers a working catalog with no other story implemented.

### Tests for User Story 1

- [X] T014 [P] [US1] Server test `only_dm_can_create_ability` in `src/server/src/graphql/mutations_abilities.rs` — a Player member is rejected, the world Owner succeeds (FR-002)
- [X] T015 [P] [US1] Server test `ability_names_may_collide` in `src/server/src/graphql/mutations_abilities.rs` — two same-named abilities in one world both insert (FR-006)
- [X] T016 [P] [US1] Server test `suggest_ability_name_finds_close_matches` in `src/server/src/graphql/queries/ability.rs` — a near match returns, an unrelated string returns empty, and the suggestion never gates create (FR-007)
- [X] T017 [P] [US1] Server test `update_ability_can_clear_description` in `src/server/src/graphql/mutations_abilities.rs` — pins the fix for the item version's defect where `description.or(existing)` makes clearing impossible (research.md §3, defect 1)
- [X] T018 [P] [US1] Server test `world_abilities_returns_all_abilities_for_a_member` in `src/server/src/graphql/queries/ability.rs` (FR-005)
- [X] T019 [P] [US1] Unit tests for the facet resolver in `apps/web/src/utils/__tests__/abilityFacets.test.ts` covering every fallback case in contracts/ability-facets.md: undefined lookup, empty lookup, present facet, empty-string label, non-object entry, and `pluralLabel` falling back to `label` (never to `label + "s"`)
- [X] T020 [P] [US1] Playwright e2e `apps/web/e2e/abilities-compendium.spec.ts` covering quickstart.md Scenario 1 — including asserting `data-testid="compendium-coming-soon"` is absent (SC-001) and that a non-GM member sees no create/edit affordances

### Implementation for User Story 1

- [X] T021 [US1] Implement `worldAbilities` (escaped `ILIKE` search, `name ASC`, `moderation::filter_visible`), `ability` (moderated-placeholder path), and `suggestAbilityName` (`similarity() > 0.4`, `LIMIT 5`) in `src/server/src/graphql/queries/ability.rs` — **all three MUST filter `gm_only = false` unless the caller is a DM (FR-024b)**, and `ability`'s rejection for a GM-only id MUST be indistinguishable from a nonexistent id so non-DMs cannot probe for hidden abilities
- [X] T022 [US1] Implement `createAbility`, `updateAbility`, and `deleteAbility` in `src/server/src/graphql/mutations_abilities.rs` per contracts/graphql-abilities.md — `CreateAbilityInput` accepts an optional `gmOnly` (default false); `UpdateAbilityInput` MUST carry `clearDescription` so a description can actually be cleared, and MUST NOT carry `gmOnly` (visibility is DM-gated via its own mutation — see US5)
- [X] T023 [P] [US1] Create `apps/web/src/utils/abilityFacets.ts` with `DEFAULT_ABILITY_FACETS`, `resolveAbilityLabel`, and `resolveAbilityPluralLabel` per contracts/ability-facets.md — modeled on `apps/web/src/utils/sizeCategory.ts`, system-agnostic, never importing a specific pack, every lookup total
- [X] T024 [P] [US1] Create `apps/web/src/api/abilities.ts` with `getWorldAbilities`, `getAbility`, `suggestAbilityName`, `createAbility`, `updateAbility`, `deleteAbility` — match each operation's argument shape to the resolver written in T021/T022 (research.md §5: write the resolver first, then the query string)
- [X] T025 [US1] Create `apps/web/src/pages/world/compendium/AbilityCompendiumTab.tsx` mirroring `ItemCompendiumTab.tsx` — server-side search, 300ms-debounced "did you mean?" hint, GM-gated create form, per-row edit gating on `myPermissionLevel !== "VIEWER"`; omit the dead `refreshKey` prop the item version declares but never receives
- [X] T026 [US1] Create `apps/web/src/pages/world/compendium/AbilityPreviewPanel.tsx` mirroring `ItemPreviewPanel.tsx`, rendering the classification through `resolveAbilityLabel`
- [X] T027 [US1] Replace the `{ value: "abilities", … content: <ComingSoonTab label="Abilities" /> }` entry with the two-panel `grid gap-4 lg:grid-cols-[2fr_1fr]` layout in `apps/web/src/pages/world/compendium/WorldCompendiumPage.tsx`, adding `selectedAbilityId`/`abilityCatalog` state alongside the existing item state
- [X] T028 [US1] Load the active system's `abilityFacets` in `apps/web/src/pages/world/compendium/WorldCompendiumPage.tsx` via `getGameSystemManifest`, mirroring `TokenPanel.tsx`'s `sizeCategories` effect (keyed on `gameSystemId`, `active` cancel flag, `.catch(() => undefined)`), and pass the lookup down to the tab and preview panel
- [X] T029 [US1] Create `apps/web/src/pages/world/ability/AbilityDetailPage.tsx` with `mode: "view" | "edit"` mirroring `ItemDetailPage.tsx` — including the moderated-content banner, the VIEWER-in-edit-mode `<Navigate>` guard, and the classification picker rendering facet labels
- [X] T030 [US1] Register `/world/:id/ability/:abilityId/view` and `/world/:id/ability/:abilityId/edit` in `apps/web/src/routes/AppRoutes.tsx` with `abilityView`/`abilityEdit` entries in `apps/web/src/routes/pageLoaders.ts`
- [X] T031 [P] [US1] Add an `abilityFacets` block to `packs/systems/genie/system.json` re-labelling at least one classification, as a live demonstration that facets work end to end (FR-010)
- [X] T032 [US1] Verify per Constitution Principle V: `cargo test -p thunderforge ability`, `npx tsc --noEmit --ignoreDeprecations 6.0`, `npx eslint --max-warnings=0` on new files, `npx vite build`, and quickstart.md Scenarios 1 and 6 against a running dev stack

**Checkpoint**: The Compendium has zero placeholder tabs (SC-001). A GM can author and find abilities, labelled in their system's vocabulary. **This is a shippable MVP.**

---

## Phase 4: User Story 2 - A GM records what an ability actually does (Priority: P2)

**Goal**: Structured, system-agnostic Effects on an ability — inert authored data for a future resolution engine.

**Independent Test**: Add two effects of different types to an ability, confirm both persist independently, edit one without disturbing the other, remove one, and confirm an invalid formula is rejected.

### Tests for User Story 2

- [X] T033 [P] [US2] Server test `add_ability_effect_rejects_empty_formula` in `src/server/src/graphql/mutations_abilities.rs` — a whitespace-only formula errors before any write (FR-018)
- [X] T034 [P] [US2] Server test `ability_can_carry_multiple_effects` in `src/server/src/graphql/mutations_abilities.rs` — two effects added independently, editing one leaves the other untouched (FR-017)
- [X] T035 [P] [US2] Server test `ability_effect_formula_is_not_evaluated` in `src/server/src/graphql/mutations_abilities.rs` — asserts effects are stored verbatim and no resolution occurs (FR-019)

### Implementation for User Story 2

- [X] T036 [US2] Create migration `src/server/migrations/<ts>_create_world_ability_effects/{up,down}.sql` per data-model.md §2
- [X] T037 [US2] Add the `world_ability_effects` `table!` block, joinable, and `allow_tables_to_appear_in_same_query!` entry in `src/server/src/schema.rs`, plus `AbilityEffect`/`NewAbilityEffect` structs in `src/server/src/models.rs`
- [X] T038 [US2] Add `AbilityEffectType`, `AbilityEffectTrigger`, and `GraphQLAbilityEffect` in `src/server/src/graphql/types.rs`, and the `effects` field on `GraphQLAbility` (fallback to `Modifier` on an unknown DB string, mirroring `GraphQLItemEffect`)
- [X] T039 [US2] Implement `addAbilityEffect`, `updateAbilityEffect`, `removeAbilityEffect`, plus private `validate_formula`/`validate_target` (structural only — non-empty, ≥1 alphanumeric, never ruleset-aware) in `src/server/src/graphql/mutations_abilities.rs`
- [X] T040 [US2] Add `addAbilityEffect`, `updateAbilityEffect`, `removeAbilityEffect` and the `ItemEffect`-shaped selection set to `apps/web/src/api/abilities.ts`, and `AbilityEffectRecord`/`AbilityEffectInput` types to `apps/web/src/types/ability.ts`
- [X] T041 [US2] Create `apps/web/src/pages/world/ability/AbilityEffectEditor.tsx` mirroring `ItemEffectEditor.tsx` and mount it in `AbilityDetailPage.tsx` — **gated on `canEdit`**, unlike the item version which renders for VIEWERs in view mode (research.md §3, defect 5)
- [X] T042 [US2] Render effects in `AbilityPreviewPanel.tsx` and verify quickstart.md Scenario 2 against a running dev stack

**Checkpoint**: Abilities carry structured mechanical data. US1 and US2 both work independently.

---

## Phase 5: User Story 3 - An actor knows abilities (Priority: P2)

**Goal**: Attach abilities to actors, with permission following the **actor**, not the ability.

**Independent Test**: Attach two abilities to an actor, confirm both appear, re-attach one and confirm no duplicate, detach one and confirm the ability still exists in the Compendium.

### Tests for User Story 3

- [X] T043 [P] [US3] Server test `actor_ability_permission_follows_actor_not_ability` in `src/server/src/graphql/mutations_actor_abilities.rs` — Editor on the actor + Viewer on the ability succeeds; Owner on the ability + Viewer on the actor is rejected (FR-022)
- [X] T044 [P] [US3] Server test `attaching_same_ability_twice_is_a_noop` in `src/server/src/graphql/mutations_actor_abilities.rs` — one row, no error, returns the existing entry (FR-021)
- [X] T045 [P] [US3] Server test `attaching_cross_world_ability_is_rejected` in `src/server/src/graphql/mutations_actor_abilities.rs` — neither the FKs nor the UNIQUE constraint prevent this, so it needs an explicit guard
- [X] T046 [P] [US3] Server test `deleting_an_ability_tombstones_actor_entries_instead_of_blocking` in `src/server/src/graphql/mutations_actor_abilities.rs` — delete succeeds, entry survives with null `ability_id` and an intact name snapshot (FR-023)
- [X] T047 [P] [US3] Server test `gm_only_abilities_are_omitted_from_a_non_dms_known_list` in `src/server/src/graphql/mutations_actor_abilities.rs` — a DM sees the entry, a Viewer-on-actor player does not, and nothing in the player's response (placeholder, count, ordering gap) hints an entry was filtered (FR-023, FR-024b)
- [X] T048 [P] [US3] Server test `detaching_does_not_delete_the_ability` in `src/server/src/graphql/mutations_actor_abilities.rs` (US3 scenario 6)

### Implementation for User Story 3

- [X] T049 [US3] Create migration `src/server/migrations/<ts>_create_world_actor_abilities/{up,down}.sql` per data-model.md §5 — `ability_id` nullable with `ON DELETE SET NULL`, `ability_name_snapshot NOT NULL`, `UNIQUE (actor_id, ability_id)`, and **no quantity column**
- [X] T050 [US3] Add the `world_actor_abilities` `table!` block, joinables, and `allow_tables_to_appear_in_same_query!` entry in `src/server/src/schema.rs`, plus `ActorAbilityEntry`/`NewActorAbilityEntry` structs in `src/server/src/models.rs`
- [X] T051 [US3] Add `GraphQLActorAbilityEntry` (with nullable `abilityId`/`classification` and non-null `abilityName`) in `src/server/src/graphql/types.rs`
- [X] T052 [US3] Create `src/server/src/graphql/mutations_actor_abilities.rs` with `actorAbilities` query, `attachAbilityToActor`, and `detachAbilityFromActor` per contracts/graphql-actor-abilities.md — all permission checks against the **actor**, plus the cross-world guard, `ON CONFLICT DO NOTHING` de-duplication, and a `gm_only` join-filter on `actorAbilities` for non-DM callers (FR-023); register it in `src/server/src/graphql.rs`'s `QueryRoot`/`MutationRoot`
- [X] T053 [P] [US3] Create `apps/web/src/api/actorAbilities.ts` and `apps/web/src/types/actorAbility.ts`
- [X] T054 [US3] Create `apps/web/src/pages/world/actor/ActorAbilitiesPanel.tsx` mirroring `ActorInventoryPanel.tsx` — catalog fetched only when `canManage`, non-optimistic refresh after each mutation, list always visible, tombstoned rows marked "(deleted ability)", classifications through `resolveAbilityLabel`
- [X] T055 [US3] Mount `<ActorAbilitiesPanel canManage={canEdit} />` beside `ActorInventoryPanel` in `apps/web/src/pages/world/actor/ActorDetailPage.tsx` (available from the view route, matching inventory) and verify quickstart.md Scenario 3

**Checkpoint**: Abilities are attached to characters. US1-US3 all work independently.

---

## Phase 6: User Story 4 - Abilities cross-link with lore, both directions (Priority: P3)

**Goal**: `[[Ability Name]]` resolves from lore, and abilities show what links to them.

**Independent Test**: Write a lore entry linking to an ability, confirm it resolves and navigates; open the ability and confirm the entry appears in its linked-from list; delete the ability and confirm the link renders broken rather than blocking the delete.

**Depends on**: T002-T003 (Phase 1 bug fixes) — a fourth link kind cannot be labelled correctly while a binary ternary decides the label.

### Tests for User Story 4

- [X] T056 [P] [US4] Server test `resolves_link_to_existing_ability` in `src/server/src/markdown/links.rs` (FR-028)
- [X] T057 [P] [US4] Server test `item_wins_over_ability_on_title_collision` in `src/server/src/markdown/links.rs` — pins the append-last precedence (research.md §4). Also add the missing `actor_wins_over_item_on_title_collision` test the existing suite lacks
- [X] T058 [P] [US4] Server test `deleting_an_ability_nulls_referencing_lore_links_instead_of_blocking` in `src/server/src/graphql/mutations_abilities.rs` — delete succeeds, link row survives with a null FK, source entry untouched (FR-031)
- [X] T059 [P] [US4] Server test `lore_link_targets_includes_abilities` in `src/server/src/graphql/queries/lore.rs` — an ability candidate is returned with kind `ABILITY` (FR-030)

- [X] T060 [P] [US4] Server test `duplicate_ability_names_resolve_to_the_oldest` in `src/server/src/markdown/links.rs` — two same-named abilities; the link resolves to the earlier-created one, stably across repeated reads (FR-030a)
- [X] T061 [P] [US4] Server test `gm_only_ability_is_unresolved_for_a_non_dm_reader` in `src/server/src/markdown/links.rs` — the same lore entry renders a working link for a DM and an unresolved span for a player (FR-030b)

### Implementation for User Story 4

- [X] T062 [US4] Create migration `src/server/migrations/<ts>_add_ability_target_to_world_lore_links/{up,down}.sql` per data-model.md §6 — four operations mirroring spec 013's item migration; `down.sql` reverses all four. Do NOT tighten the "at most one target" CHECK (contracts/ability-lore-links.md)
- [X] T063 [US4] Add `target_ability_id` to the `world_lore_links` `table!` block (**appended last, after `created_at`** — ALTER order) and a joinable in `src/server/src/schema.rs`, plus the field on `LoreLink` and `NewLoreLink` in `src/server/src/models.rs` (field order must match `schema.rs`)
- [X] T064 [US4] Add `PreparedLink.target_ability_id` (and `None` in the other constructions) and append the ability branch to the resolution cascade with kind `"ability"` and href `/world/{world_id}/ability/{ability_id}/view` in `src/server/src/markdown/links.rs` — **appended last**, after the item branch. Add `ORDER BY created_at ASC LIMIT 1` for deterministic duplicate-name resolution (FR-030a; apply the same fix to the existing lore/actor/item branches, which share the bug) and `AND (NOT gm_only OR :viewer_is_dm)` for GM-only filtering (FR-030b). **This makes resolution viewer-dependent, which the current resolver is not** — thread the viewer's DM status through the render path; lore `rendered_html` is re-rendered per read, so this is achievable. Largest single implementation cost in US4
- [X] T065 [US4] Pass `target_ability_id` through `replace_lore_links` into `NewLoreLink` in `src/server/src/graphql/mutations_lore.rs`
- [X] T066 [US4] Add `lore_entries_linking_to_ability` (a verbatim copy of `lore_entries_linking_to_item`, including its `moderation::filter_visible` pass), the `ABILITY` variant on `GraphQLLoreLinkTargetKind`, and a fourth `results.extend(...)` branch in `lore_link_targets_impl` (filtering `gm_only` for non-DM authors, FR-024b) — all in `src/server/src/graphql/queries/lore.rs`
- [X] T067 [US4] Add the `linkedFromLore` `#[graphql(complex)]` field on `GraphQLAbility` in `src/server/src/graphql/types.rs` (use `linkedFromLore`, matching `GraphQLItem`'s newer convention, not the actor's older `loreLinkedFrom`)
- [X] T068 [US4] Add `linkedFromLore { id title slug }` to the ability selection set in `apps/web/src/api/abilities.ts` and the field to `WorldAbilityRecord` in `apps/web/src/types/ability.ts`
- [X] T069 [US4] Add a "Linked from (lore)" card to `apps/web/src/pages/world/ability/AbilityDetailPage.tsx` mirroring `ItemDetailPage.tsx`'s
- [X] T070 [US4] Verify quickstart.md Scenario 4 against a running dev stack — including that all four target kinds display distinct, correct labels in the `[[` autocomplete

**Checkpoint**: The Compendium is fully cross-linked. All four content types are valid link targets.

---

## Phase 7: User Story 5 - Per-ability access control (Priority: P3)

**Goal**: A GM can grant specific members Viewer/Editor/Owner on an individual ability.

**Independent Test**: As a GM, restrict an ability, confirm from a second member's session that it is inaccessible, then grant Viewer and confirm it becomes visible.

**Note**: Enforcement already exists from Phase 2 (T008). This story delivers the management surface.

### Tests for User Story 5

- [X] T071 [P] [US5] Server test `only_dm_can_set_or_view_ability_permissions` in `src/server/src/graphql/mutations_ability_permissions.rs` — a non-DM is rejected; a DM's grant persists with the correct level string (FR-026)
- [X] T072 [P] [US5] Server test `removing_a_permission_reverts_to_implicit_viewer` in `src/server/src/graphql/mutations_ability_permissions.rs` — idempotent delete (FR-024)
- [X] T073 [P] [US5] Server test `ability_detail_is_denied_without_viewer_access` in `src/server/src/graphql/queries/ability.rs` — verifies enforcement server-side, independent of any UI (FR-025, SC-004)

- [X] T074 [P] [US5] Server test `only_dm_can_set_gm_only` in `src/server/src/graphql/mutations_abilities.rs` — an Editor and an ability-level Owner are both rejected; a DM succeeds (FR-024c)
- [X] T075 [P] [US5] Server test `gm_only_ability_is_absent_from_every_non_dm_surface` in `src/server/src/graphql/queries/ability.rs` — the leak sweep: `worldAbilities`, its search, `ability`, and `suggestAbilityName` all exclude it for a non-DM, and `ability`'s rejection is indistinguishable from a nonexistent id (FR-024b, FR-025, SC-004a)

### Implementation for User Story 5

- [X] T076 [US5] Create `src/server/src/graphql/mutations_ability_permissions.rs` with the `abilityPermissions` query, `setAbilityPermission` (UPSERT on the `(ability_id, user_id)` conflict target), `removeAbilityPermission`, and the private `require_dm_of_abilitys_world` guard; register it in `src/server/src/graphql.rs`'s `QueryRoot`/`MutationRoot`
- [X] T077 [P] [US5] Add `getAbilityPermissions`, `setAbilityPermission`, `removeAbilityPermission` to `apps/web/src/api/abilities.ts`
- [X] T078 [US5] Create `apps/web/src/pages/world/ability/AbilityOwnershipBlock.tsx` mirroring `ItemOwnershipBlock.tsx` — world creator synthesised into the subject list, `""` option meaning "Default (Viewer)" which calls remove, optimistic local patch after each await
- [X] T079 [US5] Implement `setAbilityGmOnly(abilityId, gmOnly)` in `src/server/src/graphql/mutations_abilities.rs`, DM-gated via `require_dm_of_abilitys_world` — deliberately its own mutation rather than a field on `UpdateAbilityInput`, since `updateAbility` only requires Editor and would otherwise let an Editor un-hide a GM's secret ability. Follows the existing `updateSceneHidden` precedent
- [X] T080 [US5] Add `setAbilityGmOnly` to `apps/web/src/api/abilities.ts`, a DM-only GM-only toggle plus a clear "GM-only" badge on `apps/web/src/pages/world/ability/AbilityDetailPage.tsx` (FR-024d), and a GM-only marker on the row in `apps/web/src/pages/world/compendium/AbilityCompendiumTab.tsx`
- [X] T081 [US5] Mount `{isDm && mode === "edit" ? <AbilityOwnershipBlock … /> : null}` in `apps/web/src/pages/world/ability/AbilityDetailPage.tsx`
- [X] T082 [US5] Verify quickstart.md Scenario 5 against a running dev stack — **both halves**: 5a (edit rights) and 5b's full GM-only leak checklist, including the server-side and probe-resistance checks (SC-004a)

**Checkpoint**: US1-US5 complete. The feature is fully functional within a single world.

---

## Phase 8: User Story 6 - Sharing an ability with another world (Priority: P3)

**Goal**: Share links with read-only preview and Copy-to-World.

> ### Gated on T001 — the guardrail determination must be accepted first
>
> Constitution v1.1.0's DMCA guardrail requires an on-record "centralized public repository" determination **before implementation begins**. ADR-049 drafts it (finding: share links are **not** such a repository), covering actor and item shares retroactively — none had ever been recorded. T001 is its acceptance.
>
> **The finding is conditional on six invariants** (contracts/ability-share.md). Every task below is designed to satisfy them; the load-bearing ones here are T091's v4-derived codes, its deliberate absence of any list-shares query (FR-037), and T086's moderation-bypass test.
>
> **US1-US5 have no dependency on this phase and ship without it.**


### Tests for User Story 6

- [ ] T083 [P] [US6] Server test `create_ability_share_link_requires_owner_level` in `src/server/src/graphql/mutations_ability_shares.rs` (FR-032)
- [ ] T084 [P] [US6] Server test `copy_produces_independent_ability_with_cloned_effects` in `src/server/src/graphql/mutations_ability_shares.rs` — non-DM at the destination rejected; the copy has a new id, the destination `world_id`, re-parented effects, and an empty ownership block (FR-035, SC-008)
- [ ] T085 [P] [US6] Server test `revoked_share_link_is_unavailable` in `src/server/src/graphql/mutations_ability_shares.rs` (FR-036)
- [ ] T086 [P] [US6] Server test `shared_ability_is_unavailable_once_moderation_disabled` in `src/server/src/graphql/mutations_ability_shares.rs` — a share must never be a moderation bypass
- [ ] T087 [P] [US6] Server test `shared_ability_preview_omits_source_world_identity` in `src/server/src/graphql/mutations_ability_shares.rs` (FR-033)

### Implementation for User Story 6

- [ ] T088 [US6] Create migration `src/server/migrations/<ts>_create_world_ability_shares/{up,down}.sql` per data-model.md §4 — app-supplied PK, `share_code VARCHAR(32) NOT NULL UNIQUE`
- [ ] T089 [US6] Add the `world_ability_shares` `table!` block, joinables, and `allow_tables_to_appear_in_same_query!` entry in `src/server/src/schema.rs`, plus `AbilityShare`/`NewAbilityShare` structs in `src/server/src/models.rs`
- [ ] T090 [US6] Add `GraphQLAbilityShareLink` and `SharedAbilityPreview` (deliberately carrying no `id`/`worldId`/`createdBy`/ownership block) in `src/server/src/graphql/types.rs`
- [ ] T091 [US6] Create `src/server/src/graphql/mutations_ability_shares.rs` with `sharedAbility`, `createAbilityShareLink`, `revokeAbilityShareLink`, `copySharedAbilityToWorld` per contracts/ability-share.md — reuse `generate_share_code()`'s **v4**-derived code (never v7 — spec 005 fixed a real same-millisecond collision bug), re-validate effect formulas on copy, and add **no** list-shares query (FR-037); register it in `src/server/src/graphql.rs`
- [ ] T092 [P] [US6] Create `apps/web/src/api/abilityShares.ts` and `apps/web/src/types/abilityShare.ts`
- [ ] T093 [US6] Create `apps/web/src/pages/ability-share/SharedAbilityPage.tsx` mirroring `SharedItemPage.tsx` — login required but not world membership, `idle → confirming → copying → done` step machine, empty-`dmWorlds` message, classifications rendered with **default** labels (the preview has no world context by design)
- [ ] T094 [US6] Register `/shared/ability/:code` in `apps/web/src/routes/AppRoutes.tsx` with a `sharedAbility` entry in `apps/web/src/routes/pageLoaders.ts`, and add Owner-gated Share/Revoke controls to `apps/web/src/pages/world/ability/AbilityDetailPage.tsx`
- [ ] T095 [US6] Verify quickstart.md Scenario 7 against a running dev stack, including the enumeration check that no query returns a world's or user's share links

**Checkpoint**: All six user stories complete.

---

## Phase 9: Polish & Cross-Cutting Concerns

- [ ] T096 [P] Delete `apps/web/src/pages/world/compendium/ComingSoonTab.tsx` — it loses its last caller at T027 (research.md §7)
- [ ] T097 [P] Extract the privately-duplicated `postGraphQL` helper (now in `api/items.ts`, `api/itemShares.ts`, `api/inventory.ts`, `api/abilities.ts`, `api/actorAbilities.ts`, `api/abilityShares.ts`) into one shared module under `apps/web/src/api/`
- [ ] T098 [P] De-duplicate the `EFFECT_TYPE_LABELS` map (currently copied in `ItemPreviewPanel.tsx` and `SharedItemPage.tsx`, and again for abilities) into a shared constant
- [ ] T099 Run the full `cargo test -p thunderforge` suite with `.env` loaded and containers up; confirm zero regressions against the pre-feature baseline
- [ ] T100 Run `npx tsc --noEmit --ignoreDeprecations 6.0`, `npx eslint --max-warnings=0`, and `npx vite build` in `apps/web`; confirm no new errors beyond the documented pre-existing baseline
- [ ] T101 Run `npx playwright test e2e/abilities-compendium.spec.ts --workers=1` repeatedly (at least 3 consecutive clean runs) to confirm it is not flaky
- [ ] T102 Execute quickstart.md Scenarios 1-6 end to end and tick the Definition of Done checklist
- [ ] T103 Update `specs/025-world-abilities-compendium/spec.md` Status from Draft to Implemented, and record any deviations discovered during implementation in the relevant contract files

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies. T001 (guardrail determination) gates only US6. T002-T003 are independent bug fixes; only US4 strictly requires them. All three can be done immediately, in parallel with each other.
- **Foundational (Phase 2)**: Depends on nothing in Phase 1. **BLOCKS all user stories.**
- **US1 (Phase 3)**: Depends on Phase 2 only.
- **US2 (Phase 4)**: Depends on Phase 2. Touches `mutations_abilities.rs`/`AbilityDetailPage.tsx` which US1 creates — sequence after US1 or coordinate.
- **US3 (Phase 5)**: Depends on Phase 2. Independent of US2 (different files) — can run fully in parallel with it.
- **US4 (Phase 6)**: Depends on Phase 2 **and Phase 1 (T002-T003)**.
- **US5 (Phase 7)**: Depends on Phase 2. Independent of US2/US3/US4 for its ownership-block half. **Its GM-only half (T071-T072, T080-T081) touches query filters that US1, US3, and US4 also own** — the `gm_only` filter must land in `worldAbilities`/`ability`/`suggestAbilityName` (US1), `actorAbilities` (US3), and link resolution (US4). Either build US5's GM-only half first and have the other stories filter from the start, or budget a filtering pass across all three when it lands.
- **US6 (Phase 8)**: Depends on Phase 2, US2 (clones effects), and **T001's guardrail determination being accepted**. If T001 is rejected, US6 drops and the other five stories are unaffected.
- **Polish (Phase 9)**: Depends on all desired stories. T096 depends on T027.

### Parallel Opportunities

- T002, T003 in parallel (different files).
- T012, T013 in parallel with each other within Phase 2.
- All of T014-T020 (US1 tests) in parallel.
- T023, T024, T031 in parallel (different files).
- **US3 (Phase 5) and US2 (Phase 4) can be staffed in parallel** — `mutations_actor_abilities.rs` vs `mutations_abilities.rs`, `ActorAbilitiesPanel.tsx` vs `AbilityEffectEditor.tsx`.
- **US5 (Phase 7) can run in parallel with US2/US3/US4** — its own module and component.
- All test tasks within any one story phase are parallel.
- T096, T097, T098 in parallel.

### Within Each User Story

- Tests before implementation.
- Migration → schema → models → GraphQL types → resolvers → api layer → components.
- Resolver signature before the frontend query string (research.md §5 — the argument-shape bug class).

---

## Parallel Example: User Story 1 tests

```bash
Task: "T014 Server test only_dm_can_create_ability"
Task: "T015 Server test ability_names_may_collide"
Task: "T016 Server test suggest_ability_name_finds_close_matches"
Task: "T017 Server test update_ability_can_clear_description"
Task: "T018 Server test world_abilities_returns_all_abilities_for_a_member"
Task: "T019 Unit tests for the facet resolver"
Task: "T020 Playwright e2e abilities-compendium.spec.ts"
```

## Parallel Example: US2 alongside US3

```bash
# Different files — no conflict:
Task: "T039 Implement ability effect mutations"        # US2
Task: "T052 Implement actor-ability mutations"         # US3
```

---

## Implementation Strategy

### MVP First (User Story 1 only)

1. Phase 2: Foundational (T004-T013) — **blocks everything**
2. Phase 3: User Story 1 (T014-T032)
3. **STOP and VALIDATE**: quickstart.md Scenarios 1 and 6
4. Ship — the Compendium now has zero placeholder tabs (SC-001)

Phase 1 (T002-T003) can be done at any point before US4; doing it first is cheap and fixes a live item bug.

### Incremental Delivery

1. Foundational → abilities exist and are permission-checked
2. **+ US1 → MVP: a working, system-labelled ability catalog**
3. + US2 → abilities carry mechanical data
4. + US3 → characters know abilities
5. + US4 → the Compendium is fully cross-linked
6. + US5 → per-ability access control
7. + US6 → sharing (**only after T001's determination is accepted**)

Each increment adds value without breaking the previous ones.

### Parallel Team Strategy

After Phase 2 completes:

- Developer A: US1 (largest, MVP-critical) → then US2
- Developer B: US3 (independent files)
- Developer C: US5, then US4 (after T002-T003)
- US6 stays unstaffed until the guardrail determination lands

---

## Notes

- `[P]` = different files, no dependencies on incomplete tasks.
- Every DB-backed test needs `set -a && source .env && set +a` plus running containers.
- This feature has **no Bevy canvas surface**, so its Playwright coverage is genuinely runnable in this sandbox — unlike every canvas-interaction spec in the repo.
- Six template defects from the item implementation are deliberately **not** inherited (research.md §3); the tasks that fix them are T002, T003, T017/T022 (clearable description), T025 (no dead `refreshKey`), T041 (`canEdit`-gated editor), T091 (re-validate on copy).
- Commit after each task or logical group; stop at any checkpoint to validate a story independently.
