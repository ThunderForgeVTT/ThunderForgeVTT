# Tasks: Scene ownership and the role gates

**Input**: Design documents from `specs/093-scene-ownership-and-role-gates/`
**Prerequisites**: plan.md, spec.md (the mutation matrix is the gate list)
**Tests**: Every gate is proven server-side first (a Rust test that calls the
mutation as the refused role and as the admitted one), then by the slice.
A hidden button is never the proof.
**Proof**: slice **scenes** (`pnpm e2e:scenes`) plus the neighbours
`pnpm e2e:which` names (expected: interactive, combat, tokens, actors,
accounts). Recorded in T063.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an
  unfinished task)
- **[Story]**: US1 (Trusted Player runs the fight), US2 (a player owns their
  scene), US3 (a hidden scene shown to one player); cross-cutting tasks
  carry no story
- Every task names its file(s)

## Phase 1: Authz foundation

- [ ] T001 Add `Role::runs_the_table()` (`>= TrustedPlayer`) in
      `crates/thunderforge-authz/src/role.rs`; update the test whose message
      reads "trusted with content, not with the table" to assert the new
      line: a Trusted Player runs the table but not the world (FR-001,
      FR-007).
- [ ] T002 Add `Capability::RunTheTable` mapped to `runs_the_table` in
      `crates/thunderforge-authz/src/capability.rs`; extend the full-matrix
      test with the new row for all four roles and the site admin (FR-001).
- [ ] T003 [P] Change `scene_visible(runs_the_world, scene)` to
      `scene_visible(runs_the_world, holds_viewer, scene)` in
      `crates/thunderforge-authz/src/scene.rs`; tests for hidden+viewer,
      hidden+no grant, hidden+active (FR-014).
- [ ] T004 [P] Audit every caller of `scene_visible` and every `hidden`
      filter in `crates/thunderforge-server/src/graphql/queries/`,
      `subscriptions.rs`, `assets_serve/scene.rs` and
      `crates/thunderforge-engine`'s scene loading; list them in a comment
      block at the top of `auth/scene_visibility.rs` (created in T015) so
      T015 to T017 have a checklist (FR-014, plan risk 2).

**Checkpoint**: `cargo test -p thunderforge-authz` green; the server does not
compile yet (signature change), which is intended.

## Phase 2: Schema

- [ ] T005 Migration `2026-10-14-100000-0000_world_scene_permissions/up.sql`
      and `down.sql` in `crates/thunderforge-server/migrations/`: table per
      FR-008 (`ON DELETE CASCADE` to `scenes` and `users`, CHECK on level,
      UNIQUE on `(scene_id, user_id)`, `created_by`/`updated_by` FKs,
      indexes on `scene_id` and `user_id`).
- [ ] T006 [P] Migration `2026-10-14-100100-0000_backfill_world_owner_members`:
      `INSERT INTO world_members (id, world_id, user_id, role, joined_at,
    created_at, updated_at) SELECT ... FROM worlds w WHERE NOT EXISTS (...)
    ON CONFLICT DO NOTHING`; `down.sql` deletes only rows whose
      `created_at` equals the migration's marker timestamp column value
      (write the marker into `joined_at = created_at = now()` and record the
      run time in a one-row comment so `down` is precise) (FR-022).
- [ ] T007 [P] Migration `2026-10-14-100200-0000_drop_policies`: `up.sql`
      drops `policies` and type `PolicyEffect`; `down.sql` recreates both as
      `2026-05-02-022200-0000_create_policies` plus the two later `ALTER`s
      did, empty (FR-018).
- [ ] T008 Regenerate `crates/thunderforge-server/src/schema.rs` (diesel
      print-schema per CONTRIBUTING.md); confirm `world_scene_permissions`
      is present, `policies` and `sql_types::PolicyEffect` are gone, and
      `allow_tables_to_appear_in_same_query!` lists the new table.

**Checkpoint**: migrations apply and revert cleanly on `thunderforge_test`
(`diesel migration run`, `redo`); `schema.rs` diff is only those three
changes.

## Phase 3: Scene permission validators (US2, US3)

- [ ] T009 [US2] Add the fifth arm `scene { effective:
    effective_scene_permission, require: require_scene_permission, purge:
    purge_scene_grants, grants: world_scene_permissions, content_fk:
    scene_id, user_fk: user_id, parent: scenes, noun: "scene",
    noun_capitalized: "Scene" }` to `permissioned_entities!` in
      `crates/thunderforge-server/src/auth/permissioned_entities.rs`; the
      generated `purge_member_grants` now walks five tables (FR-009, FR-011).
- [ ] T010 [US2] Rust test in `permissioned_entities.rs` (or its `_tests`
      sibling): the GM resolves Owner on any scene with an empty block; a
      member with no row resolves Viewer; a removed member's scene row is
      gone after `purge_member_grants` (US2 scenario 5).
- [ ] T011 [US2] Rename `is_dm_of_scene` to `may_author_scene` in
      `crates/thunderforge-server/src/auth/world_membership.rs`: true when
      `actor_in_world(..).runs_the_world()` or
      `effective_scene_permission >= Editor`; keep the "a DB hiccup can only
      deny" shape (FR-012).
- [ ] T012 [US2] Add `may_run_scene(conn, user, is_admin, scene_id)` beside
      it: true when `runs_the_table()` or scene permission is Owner
      (FR-013); and `may_edit_scene_block(..)` = `runs_the_world()` only
      (FR-010).
- [ ] T013 [US2] Fix every former `is_dm_of_scene` caller (compile-driven):
      `mutations_walls.rs`, `mutations_lighting.rs`, `mutations_shapes.rs`
      (`shape_authority`'s Dm branch), `mutations_tokens.rs` (create,
      delete, the full-update branch), `mutations_levels.rs` /
      `scene_levels.rs:151`, `mutations_interactives.rs` (create, update,
      delete, reset, designation, secret), `mutations_scenes.rs`
      (`updateScene`, `deleteScene` to `may_run_scene`, `updateFogMask`),
      `mutations_party.rs` (`bringPartyToScene` to `may_run_scene`),
      `uploadCanvasImage`'s resolver (FR-012, FR-013).
- [ ] T014 [US2] Move `updateSceneHidden`, `updateSceneAmbientLight`,
      `setSceneExploration`, `resetSceneExploration` from `is_dm_of_world`
      to `may_author_scene`, and `launchScene` to `may_run_scene`, in
      `mutations_scenes.rs` and `exploration.rs` (FR-012, FR-013).
- [ ] T015 [US3] New `crates/thunderforge-server/src/auth/scene_visibility.rs`:
      `visible_scene_ids(conn, user, is_admin, world_id) -> Vec<Uuid>` doing
      one query that joins `scenes` with the caller's grants and the world's
      `active_scene_id`, calling `scene_visible` per row; export a
      `scene_is_visible(conn, user, is_admin, scene_id)` for single lookups
      (FR-014).
- [ ] T016 [US3] Route `scenes(worldId)`, `scene(id)` and the scene
      subscription filter in `graphql/queries/scene.rs` and
      `graphql/subscriptions.rs` through T015; every caller from the T004
      list is converted or has a one-line reason recorded next to it
      (FR-014).
- [ ] T017 [US3] Make every scene-scoped mutation that today returns
      `NotFound` for an unauthorized caller keep doing so for a caller
      refused by visibility (no new error text); add a test that a Player
      mutating a token on a hidden scene they hold no grant on gets the same
      answer as for a random UUID (FR-015).
- [ ] T018 [US2] Rust tests in `mutations_scenes.rs` / `mutations_walls.rs`
      tests: Editor on A creates a wall on A and is refused on B; Editor is
      refused `launchScene` and `deleteScene`; Owner is admitted to both
      (US2 scenarios 2, 3).
- [ ] T019 [US3] Rust test in `queries/scene.rs` tests: hidden scene listed
      for a Viewer grant holder, absent for another Player, present for all
      once active (US3 scenarios 1 to 3).

**Checkpoint**: `cargo test -p thunderforge-server` green; no caller of the
old name remains (`grep is_dm_of_scene` finds only history comments).

## Phase 4: Trusted Player gates (US1)

Each task names the exact mutations, cites the matrix row, and carries a
test for both the admitted Trusted Player and the still-refused Player.

- [ ] T020 [US1] `mutations_scenes.rs`: `launchScene` admits
      `runs_the_table()` via `may_run_scene` (done in T014; add the Trusted
      Player test here) (FR-002).
- [ ] T021 [P] [US1] `mutations_tokens.rs`: `updateToken` gains a
      position-only path: if the caller is not admitted by
      `may_author_scene` but is by `may_run_scene`, accept only when the
      input's non-position fields are all absent; otherwise refuse as today.
      Tests: Trusted Player moves another player's token; Trusted Player
      with `label` present is refused; Player is refused both (FR-003).
- [ ] T022 [P] [US1] `mutations_levels.rs` / `scene_levels.rs`:
      `moveTokensToLevel` through `may_run_scene` (FR-013).
- [ ] T023 [P] [US1] `mutations_interactives.rs`: `activate_interactive_as`
      computes the GM outcome from `may_run_scene` instead of
      `runs_the_world`; `approveRequest`, `refuseRequest`, `setDoorLock`
      through `may_run_scene`; `setDoorDesignation`, `setDoorSecret` stay
      on `may_author_scene`. Tests: a Trusted Player opens a locked door, a
      Player still gets the `locked` refusal reason (FR-004).
- [ ] T024 [P] [US1] `mutations_party.rs`: Trusted Player test for
      `bringPartyToScene`; the existing `a_player_may_not_bring_the_party`
      test stays (FR-013).
- [ ] T025 [P] [US1] `mutations_combat.rs`: `startCombat`, `addCombatant`,
      `updateCombatant`, `removeCombatant`, `advanceTurn`, `endCombat`,
      `changeHitPoints`, `addLairCombatant` from `is_dm_of_world` to a
      `runs_the_table` check (new helper `is_table_runner_of_world` in
      `world_membership.rs`); `setCombatAutoApply` and
      `updateWorldAutoApplyNpcDamage` stay. Module doc comment line 9
      updated. Tests for one admitted and one refused role per mutation
      (FR-002).
- [ ] T026 [P] [US1] `graphql/actor_conditions.rs`: `applyActorCondition`,
      `clearActorCondition` through `is_table_runner_of_world` (FR-002).
- [ ] T027 [P] [US1] `mutations_roll.rs`: `revealRoll` through
      `is_table_runner_of_world`; `clearWorldRolls` stays GM (FR-002).
- [ ] T028 [US1] Refusal regression test, one file
      `crates/thunderforge-server/src/graphql/trusted_player_gates_tests.rs`:
      as a Trusted Player, each of `updateMemberRole`, `removeMember`,
      `generateInviteCode`, `revokeInviteCode`, `rotateInviteCode`,
      `updateWorldGameSystem`, `renameWorld`, `deleteWorld`, `createWall`,
      `createLightSource`, `createToken`, `deleteToken`, `createSceneLevel`,
      `updateFogMask`, `setSceneExploration`, `updateSceneHidden`,
      `setTokenDisclosure`, `setTokenNameVisibility`,
      `setActorVisibleToPlayers`, `rollBackActor`, `clearWorldRolls`,
      `sendChatMessage(gmOnly)`, `setScenePermission` is refused with the
      Player's answer (FR-005, SC-002).
- [ ] T029 [US1] `mutations_reconcile.rs`: confirm the replay path reaches
      the same validators (no gate of its own); add one replayed
      `updateToken` position move as a Trusted Player to its tests
      (assumption 4).
- [ ] T030 [US1] Telemetry: every refusal site touched in T020 to T028
      records `authz.capability = "RunTheTable"` (or the capability it
      checked) and `authz.role` on the current span, using the helper the
      existing refusals use (FR-020).

**Checkpoint**: `cargo test -p thunderforge-server` green; every row marked
**changed** in the matrix has a named test.

## Phase 5: GraphQL surface

- [ ] T031 [US2] New `crates/thunderforge-server/src/graphql/mutations_scene_permissions.rs`:
      `setScenePermission(input: SetScenePermissionInput!)` and
      `removeScenePermission(sceneId, userId)`, gated by
      `may_edit_scene_block`, refusing a grant to the GM's own row and to a
      non-member; mirrors `mutations_actor_permissions.rs` line for line,
      including the world event it emits (FR-010).
- [ ] T032 [US2] `graphql/types_scene.rs`: `GraphQLScenePermission` and a
      `permissions: [GraphQLScenePermission!]!` field on `GraphQLScene`,
      resolved only for a caller who runs the world (others get `[]`)
      (FR-010, FR-017).
- [ ] T033 [US2] Register the new mutation object in
      `apps/thunderforge/src/schema_roots.rs` (or wherever
      `AppMutationRoot` merges its parts) and regenerate
      `apps/thunderforge/schema.graphql`; the diff adds the two mutations,
      one input, one type, one field, and removes `GraphQLPolicy` and the
      two `policies` fields (T038) (FR-010, FR-018).
- [ ] T034 [US2] Rust test: a Player holding Owner on A is refused
      `setScenePermission` on A (SC-005).

## Phase 6: The creator's Owner row

- [ ] T035 `mutations_worlds.rs`: `create_world_impl` and
      `create_world_with_map_impl` insert the `world_members` Owner row in
      the same transaction as the world and the Starting Scene (FR-021).
- [ ] T036 Rust test: after `createWorld`, `world_members` has exactly one
      row for the creator with role `Owner`; `require_world_member` finds it
      without the fallback (set the fallback behind a test-only flag, or
      assert the row directly) (FR-021, FR-023).
- [ ] T037 Update the doc comment on `require_world_member`
      (`world_membership.rs:51` to 63) to say the fallback is now a belt
      for un-migrated rows and name the follow-up that removes it
      (plan decision).

## Phase 7: The dead table

- [ ] T038 Remove `policies` from `admin.rs` (`total_policies` and its
      GraphQL/JSON field), `users/mod.rs` (the counts and the export
      vector), `graphql/mutations_user_data.rs`, `graphql/input_types.rs`
      (`GraphQLPolicy`, the two fields), and delete the commented-out
      blocks in `models.rs:517`, `graphql/helpers.rs:303` to 323 and 446 to
      455, `graphql/queries/user.rs:226` and `graphql.rs:18`'s trailing
      comment (FR-018).
- [ ] T039 [P] Delete `crates/thunderforge-core/src/policies/` and the
      `pub mod policies;` line in `crates/thunderforge-core/src/lib.rs`;
      `cargo build --workspace` proves nothing used it (FR-018).
- [ ] T040 [P] Check `apps/web/src` for any reader of the export's
      `policies` fields (expected none; `user-data-export.spec.ts` in the
      accounts slice is the proof) and remove it if found (SC-006).

**Checkpoint**: `grep -r policies crates/ apps/thunderforge/schema.graphql`
finds nothing; `cargo build --workspace` and `cargo test` green.

## Phase 8: Web

- [ ] T041 [US1] `apps/web/src/types/world.ts`: `runsTheTable(role)`
      (`>= TrustedPlayer`) with a doc comment pointing here;
      `apps/web/src/hooks/useWorldRole.ts`: `runsTheTable: boolean`
      beside `isGm` and `managesContent` (FR-006).
- [ ] T042 [US1] Show the table-running controls to `runsTheTable`: launch
      in `components/world/SceneSwitcher/SceneSwitcher.tsx`; combat start,
      advance, end, hit points and conditions in
      `components/world/PlayDock/**`; door lock and request approve/refuse
      in `components/canvas-tools/**`; reveal roll in the roll log. Every
      other `isGm` check is left as it is (FR-006).
- [ ] T043 [P] [US2] `apps/web/src/api/scenes.ts`: `setScenePermission`,
      `removeScenePermission` documents; `engine/world/sync/scenes.ts`
      carries `permissions` on the scene record and dispatches the grant
      world event into the store (FR-010, AGENTS.md circular flow).
- [ ] T044 [US2] `apps/web/src/pages/world/scenes/SceneOwnershipBlock.tsx`
      from the actor block component: lists every member, GM row fixed at
      Owner, select per member, rendered on the scene's page for
      `isGm` only (FR-017).
- [ ] T045 [US3] Confirm `SceneSwitcher.tsx` and `PlayDock/WorldDock.tsx`
      render only what `scenes(worldId)` returned and add no filter of their
      own; if either filters on `hidden` client-side, remove it (FR-016).
- [ ] T046 [P] Unit tests: `types/world.test.ts` for `runsTheTable` across
      the four roles and `null`; `SceneOwnershipBlock.test.tsx` renders the
      GM row disabled.

**Checkpoint**: `pnpm verify` green (typecheck, lint, unit).

## Phase 9: Documents

- [ ] T047 Append a dated amendment to
      `docs/adrs/20260913-099-the_trusted_player_role.md`: this spec
      narrows the "may not" paragraph to authoring, membership, settings,
      fog, hidden content and the world's existence; running a scene is now
      theirs; link to spec 093 (FR-007; spec open question stands).
- [ ] T048 [P] `MVP.md` Phase 10: `[~]` to `[x]`; replace the "Missing"
      paragraph with the four roles, the five ownership blocks, the Owner
      row, and the removal of `policies`; drop the `permission_grants`
      mention (FR-019).
- [ ] T049 [P] `docs/SECURITY_RBAC.md`: four roles, `runs_the_table`, the
      scene block, the Owner row now inserted (fallback to be removed in a
      follow-up), `policies` gone, `permission_grants` never existed
      (FR-019).
- [ ] T050 [P] `scripts/e2e/slices.json`: add
      `crates/thunderforge-server/src/graphql/mutations_scene_permissions.rs`,
      `auth/scene_visibility.rs`, `pages/world/scenes/SceneOwnershipBlock.tsx`
      and `graphql/trusted_player_gates_tests.rs` to the **scenes** slice's
      paths; `node scripts/check-e2e-slices.mjs` passes.

## Phase 10: Proof

- [ ] T051 [US1] `apps/web/e2e/scene-trusted-player.spec.ts`: as a Trusted
      Player, launch a scene, move another player's token through the
      engine, open a locked door, start combat and advance a turn, each seen
      by a second browser context; then assert the server refuses
      `removeMember`, `updateMemberRole`, `generateInviteCode`,
      `updateWorldGameSystem`, `renameWorld`, `deleteWorld`, `createWall`,
      `updateFogMask`, `updateSceneHidden` and `updateToken` with a label
      (direct GraphQL calls from the test, asserting the refusal), and that
      the page shows no control for them (SC-001, SC-002).
- [ ] T052 [US2] `apps/web/e2e/scene-ownership.spec.ts`: GM grants Editor
      on A; the player draws a wall on A, is refused on B; raised to Owner,
      launches and deletes A; the player holding Owner is refused
      `setScenePermission`; the GM removes the member and a direct query
      shows no grant row; a fresh world lists its creator as Owner
      (SC-003, SC-005, SC-008).
- [ ] T053 [US3] `apps/web/e2e/scene-visibility.spec.ts`: hidden A, Viewer
      to P1; P1's switcher lists A and opens it; P2's switcher and dock do
      not; P2's `scene(A)` is null; P2's token mutation on A is refused as
      not found; GM launches A and P2 sees it (SC-004).
- [ ] T054 Run `pnpm e2e:scenes --record-durations`; record the result (pass
      count, duration) here.
- [ ] T055 Run `pnpm e2e:which` for this branch's diff; run every slice it
      names (expected interactive, combat, tokens, actors, accounts);
      record names and results here. If it names a slice this plan did not
      expect, record why.
- [ ] T056 `pnpm verify` and `cargo test --workspace` (with the RustFS
      bucket present, see memory) green; record here.
- [ ] T057 Tick the FR and SC ledgers: every FR-001 to FR-023 and SC-001 to
      SC-008 maps to a passing test or a recorded document change; list any
      that does not with the reason.

**Checkpoint**: Phase 10 of MVP.md is `[x]` and the proof is recorded above.

## Dependencies

- Phase 1 before Phase 3 (signature change); Phase 2 before Phase 3 (table).
- Phase 3 before Phase 4 (`may_run_scene` is what Phase 4 gates on).
- Phases 5, 6, 7 are independent of each other and of Phase 4 after Phase 3.
- Phase 8 after Phase 5 (schema.graphql drives the SDK types).
- Phase 9 any time after Phase 7 (the documents describe the end state).
- Phase 10 last.

## Ledger

| Requirement            | Tasks                        |
| ---------------------- | ---------------------------- |
| FR-001                 | T001, T002                   |
| FR-002                 | T020, T025, T026, T027       |
| FR-003                 | T021                         |
| FR-004                 | T023                         |
| FR-005                 | T028                         |
| FR-006                 | T041, T042                   |
| FR-007                 | T001, T047                   |
| FR-008                 | T005                         |
| FR-009                 | T009, T010                   |
| FR-010                 | T012, T031, T032, T033, T043 |
| FR-011                 | T009, T010                   |
| FR-012                 | T011, T013, T014             |
| FR-013                 | T012, T013, T014, T022, T024 |
| FR-014                 | T003, T004, T015, T016       |
| FR-015                 | T017                         |
| FR-016                 | T045                         |
| FR-017                 | T032, T044                   |
| FR-018                 | T007, T008, T033, T038, T039 |
| FR-019                 | T048, T049                   |
| FR-020                 | T030                         |
| FR-021                 | T035, T036                   |
| FR-022                 | T006                         |
| FR-023                 | T036                         |
| SC-001, SC-002         | T051                         |
| SC-003, SC-005, SC-008 | T052                         |
| SC-004                 | T053                         |
| SC-006                 | T040, T056                   |
| SC-007                 | T048, T049                   |
