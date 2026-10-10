# Tasks: First Session Feedback

**Input**: Design documents from `specs/088-first-session-feedback/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md,
contracts/ (graphql.md, base-maps.md, layouts.md, telemetry.md),
quickstart.md

**Hotfix dependencies** (spec.md, Related hotfixes):

- **`hotfix-world-permissions`**: every US1 task (T010–T029) starts only
  once it is on main.
- **`hotfix-invite-uses`**: T028 (sign-in only from a join) merges after
  it, since both change `crates/thunderforge-server/src/auth/`.
- **`hotfix-player-hero-edit`**: every US8 task (T100–T107) starts only
  once it is on main.
- **`hotfix-map-load-sync`**: no task waits for it. It is what carries a
  later import's walls (T081) to a second client.

If a hotfix is not on main when its phase comes up, that phase waits and
the others go on.

**Tests**: TDD for the pure cores: the link code (T010), the clear rule
(T060), the perimeter (T075), the settings form model (T090) and the
demo's mirrors (T068, T083). Each test is written and seen failing
before its code. Each story ends on its own slice, plus whatever
`pnpm e2e:which --diff` names. The full suite is never run.

**Telemetry**: each story's event task (contracts/telemetry.md) waits for
spec 086 if it has not landed. Mark such a task `(waits on 086)` and move
on.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel. It touches a different file, and depends on
  no unfinished task.
- **[Story]**: US1 links, US2 base maps, US3 actor view, US4 world page,
  US5 clear rolls, US6 edge walls, US7 mail form, US8 hero polish.

---

## Phase 0: Setup

**Purpose**: a worktree on the right base.

- [X] T001 Check which hotfix branches are on main (`git log --oneline main` and `mcp__gitops__workspace_scan`). Record the result at the top of research.md under `## Base`.
- [X] T002 Create the worktree: `git worktree add ../ThunderForgeVTT-088 -b 088-first-session-feedback main`. Record the base commit under `## Base`.
- [ ] T003 [P] `pnpm install`, `cargo check -p thunderforge-server`, and confirm the RustFS bucket `thunderforge-canvas-assets` exists (quickstart.md, step 0).

**Checkpoint**: the worktree builds on main with the landed hotfixes.

---

## Phase 1: Foundational

**Purpose**: the migrations and the event code that several stories read.
No behaviour changes.

- [X] T005 [P] Migration M1 `world_invites_default_one` (data-model.md), with `down.sql`. Landed as `world_invites_optional_limit`, the data-model name, since the owner settled on no limit by default.
- [X] T006 [P] Migration M2 `base_map_assets` (`canvas_image_assets.base_map_id`), with `down.sql`.
- [X] T007 [P] Migration M3 `rolls_cleared_at` (`worlds.rolls_cleared_at`), with `down.sql`.
- [X] T008 Run the migrations, regenerate `crates/thunderforge-server/src/schema.rs`, and update the Diesel models that select `*` from the two tables. `cargo test -p thunderforge-server` passes unchanged. schema.rs is edited by hand (the shared dev database is left alone); the test database migrates on demand. The core crate's `WorldInvite` keeps `0` for no limit and the adapter converts.
- [X] T009 Add `EVENT_CODE_ROLLS_CLEARED: i32 = 39` to `world_events.rs`, with its doc comment, and to the web's and demo's event-code lists.

**Checkpoint**: the schema holds the three columns. Every existing test
still passes. Commit: "Spec 088: the migrations and event 39".

---

## Phase 2: User Story 1 - World links (Priority: P1)

**Goal**: GM-only, revocable links for existing accounts, with an optional
use limit counted only when someone joins, and a clear message for each
refusal.

**Depends on**: `hotfix-world-permissions` (all), `hotfix-invite-uses`
(T028).

**Independent Test**: `pnpm e2e:accounts`.

- [X] T010 [P] [US1] TDD: tests in `graphql/share_codes_tests.rs` for the Crockford encoder (26 characters, alphabet, normalising `O`/`I`/`L`) and for 1000 codes with no repeat. See them fail.
- [X] T011 [US1] `generate_link_code()` in `graphql/share_codes.rs`: 16 bytes from `OsRng`, Crockford base32. Lookup normalises before comparing (FR-006). T010 passes. `SysRng` (rand 0.10's name for the OS source), with the thread CSPRNG as fallback. Old 20-character hex codes still resolve. The content share-code tests now expect 26 characters.
- [X] T012 [P] [US1] Tests in `mutations_invites_tests.rs`: `maxUses` 0 and 51 refused; a bad `expiresAt` refused; a past one refused; no limit and 7 days when left out (FR-002, contracts/graphql.md). Migration M1 (nullable `max_uses`, data-model.md) lands with these. Tests live in `mutations_invites_link_tests.rs` (`mutations_invites_tests.rs` is near the line limit).
- [X] T013 [US1] `generateInviteCode`: the limits, the expiry parse, the defaults; the join predicate reads `max_uses IS NULL` as no limit. T012 passes.
- [X] T014 [P] [US1] Tests: `joinWorld` returns `LINK_REVOKED`, `LINK_EXPIRED`, `LINK_USED_UP` and `LINK_UNKNOWN`, each with its message, and a member re-joining, the owner, a preview and a failed join use nothing (FR-008). The use-counting tests from `hotfix-invite-transactional` (`mutations_invites_use_tests.rs`) stay green. `ALREADY_MEMBER` stays an error with that code, not a success return; the page reads it as "already in this world".
- [X] T015 [US1] `joinWorld`: the four codes in place of `LINK_UNAVAILABLE_MESSAGE` (`mutations_invites.rs:358`). The use is still taken in the membership's transaction. T014 passes.
- [X] T016 [US1] `worldByInviteCode` and `alreadyMember` (`queries/invite.rs`) need a signed-in caller, with a test for the signed-out case (FR-007). Also: a signed-in caller who is not a member gets the link's refusal code from `worldByInviteCode`, so the join page says why before any Join button; a member still gets the preview.
- [X] T017 [US1] A Tower layer on the `/join/*` and `/invite/*` SPA routes in `apps/thunderforge/src/main.rs`: `X-Robots-Tag: noindex, nofollow`, `Referrer-Policy: no-referrer`. A test asserts both headers, and that `robots.txt` does not disallow the paths (FR-011). Done in `crates/thunderforge-server/src/static_files/` (where `/join` and `/invite` are served), not `main.rs`; the test is `link_pages_are_not_indexed_and_send_no_referrer`.
- [X] T018 [US1] `apps/web/src/pages/world/players/WorldLinksPanel.tsx` (new): create (no limit or 1 to 50 uses, 1 d / 7 d / 30 d / none, defaults no limit and 7 d), list (active first, **Past links** folded), copy, and revoke with a confirmation (FR-001, FR-002, FR-004, FR-005). It reads through a fetch hook with `refetch()`, and refetches on the `world_invites` world event. Drawn only when the viewer runs the world. Revoking now records a code-2 world event (`{invite_id, revoked: true}`) so the list follows revokes; the code-2 payload no longer carries the link code, which every member could read (`link_events_name_no_code_and_a_revoke_is_announced`). Live refresh: `engine/world/sync/worldLinks.ts`.
- [X] T019 [US1] Put `WorldLinksPanel` on `PlayersPage.tsx`, and take link management out of `components/campaign/CampaignSettingsPanel.tsx` (the world page's Players card comes in T056).
- [X] T020 [P] [US1] `components/world/SessionSetupInviteLink.tsx`: the FR-002 defaults, and a link to the players page (FR-003).
- [X] T021 [P] [US1] `pages/world/JoinWorldPage.tsx`: a message per code, "You're already in this world." for a member, and no world name for a signed-out visitor (FR-008, FR-009). Refusal headings in `pages/world/joinRefusal.ts`; the messages are contracts/graphql.md's.
- [X] T022 [P] [US1] `pages/auth/LoginPage.tsx`: with a `returnTo` under `/join/`, no **Register** and a line saying world links are for existing accounts (FR-009). The sign-in screen is `LoginView.tsx` (`isWorldLinkReturn`).
- [X] T023 [US1] e2e `apps/web/e2e/world-links.spec.ts` (accounts slice; add it to `scripts/e2e/slices.json` if the slice's globs miss it):
  - a GM creates a link with the defaults; a player joins; the list shows one join;
  - a GM creates a one-use link; it is opened and refreshed, and still has its use; a player joins; a second account reads the used-up message;
  - a revoked link reads the revoked message;
  - a player sees no link panel on the players page;
  - signed out, `/join/<code>` leads to a sign-in page with no **Register**;
  - the list updates when the link is used, with no reload.
- [X] T024 [P] [US1] Extend `invite-membership.spec.ts` where it asserts the old single message or 5 uses. Also moved every spec that made a link from `/world/:id` to `/world/:id/players` (19 call sites plus `journeyTable.ts`, `access-links`, `abilities-ux`, `map-editor-tooling`, `world-administration`, three journeys).
- [ ] T025 [P] [US1] Telemetry: `world_link.created`, `world_link.revoked`, `world_link.join`, and the `thunderforge.world_links.joins` counter (contracts/telemetry.md). (waits on 086)
- [ ] T026 [US1] `pnpm e2e:accounts`, then `pnpm e2e:which --diff` and each slice it names (not the full suite: Open item 7).
- [ ] T027 [US1] Commit: "Spec 088: GM-only world links with an optional use limit and a reason for every refusal".
- [X] T028 [US1] **After `hotfix-invite-uses` is on main.** OAuth sign-in only from a join page (FR-010): the authorize step marks the state `sign_in_only` when `returnTo` is under `/join/`, and the callback (`auth/oauth.rs:394`) refuses to create a user for it, with the FR-010 message and the `thunderforge.world_links.oauth_refused` counter. Tests in `auth/` for `open`, `invite_only` and `closed` modes (SC-002). An e2e case in `world-links.spec.ts` with the mock OAuth provider. Run `pnpm e2e:accounts`. *Done: the mark is the authorization session's own `return_to` (no new column), read in the callback as `sign_in_only`; the refusal is `auth/world_link_sign_in.rs` (403 `world_link_sign_in_only`), checked before the admission gate so no invitation use is judged or burned. Tests: `auth/instance_access_tests.rs` walks `open`, `invite_only` and `closed`. The e2e case is in `oauth-provider.spec.ts` (accounts slice) beside the stub's helpers. The `thunderforge.world_links.oauth_refused` counter is (waits on 086).*
- [ ] T029 [US1] Commit: "Spec 088: a world link never creates an account".

**Checkpoint**: SC-001 and SC-002 hold. `pnpm e2e:accounts` is green.

---

## Phase 3: User Story 2 - Base maps (Priority: P1)

**Goal**: a new world opens on a chosen base map, credited.

**Independent Test**: `pnpm e2e:worlds`, then `pnpm e2e:scenes` and
`pnpm e2e:canvas`.

- [X] T030 [US2] Move `apps/demo/credit.json` to `examples/maps/credit.json`, and point `apps/demo/src/credit.ts` at it. The demo's tests pass (FR-028).
- [X] T031 [US2] A `make base-maps` target that runs `thunderforge-demo-maps examples/maps target/base-maps` and copies `credit.json`. A `base-maps` Dockerfile stage, copied to `/srv/base-maps` in `server`, with `THUNDERFORGE_BASE_MAPS_DIR` set (contracts/base-maps.md). Check that `NOTICE.txt` is in the output (FR-031).
- [X] T032 [P] [US2] `crates/thunderforge-server/src/base_maps/mod.rs` (new): load the directory at start-up, with tests for a missing directory, a bad `maps.json` and a missing image (FR-022).
- [X] T033 [US2] `--base-maps-dir` / `THUNDERFORGE_BASE_MAPS_DIR` in `apps/thunderforge/src/main.rs` (clap, `env`), into the app state.
- [X] T034 [P] [US2] `base_maps/routes.rs`: the three HTTP routes, matched by id against the loaded set, signed-in only, with the cache header. A test that `..` and an unknown id are 404.
- [X] T035 [US2] `baseMaps` and `defaultBaseMapId` queries, and the `MapCredit` type (contracts/graphql.md, FR-021).
- [X] T036 [US2] Tests for `createWorld`'s `baseMapId`: absent applies the default, `null` applies none, an unknown id is refused before anything is created, and the rescue flow applies none (FR-023, FR-026).
- [X] T037 [US2] `createWorld` (`mutations_worlds.rs`): after the world's transaction commits, apply the map as contracts/base-maps.md describes: the upload with `base_map_id`, then size, grid, walls (perimeter included, from `maps.json`), doors and lights in one transaction, and `MAP_IMPORTED`. A failure adds `STARTING_MAP_FAILED` and leaves the world (FR-024, FR-025). The `thunderforge.base_maps.applied` counter (waits on 086). T036 passes.
- [X] T038 [P] [US2] Replace the stale `STARTER_SCENE_NAME` comment (`mutations_worlds.rs:22-41`) with the licence as settled (FR-032).
- [X] T039 [US2] `GraphQLScene.backgroundCredit`, resolved through `base_map_id`, null otherwise, with a test (FR-029).
- [X] T040 [US2] `apps/web/src/pages/world/BaseMapPicker.tsx` (new): a keyboard-usable radio group of thumbnail cards plus **None**, the default selected, the credit beside it, usable at 375 px. Put it on `CreateWorldPage.tsx`, sending `baseMapId`, and show the `STARTING_MAP_FAILED` notice (FR-027, FR-030).
- [X] T041 [P] [US2] `apps/web/src/components/world/MapCredit.tsx` (new): the short credit line on the board's React chrome while the scene's `backgroundCredit` is set, expanding to the full credit with its three links. Also shown in the scene list and the scene's settings (FR-030).
- [X] T042 [US2] e2e `apps/web/e2e/world-base-map.spec.ts` (worlds slice):
  - create with the default: the board shows the map and the credit, with no step after Create;
  - create with **None**: the Starting Scene matches today's, field for field (SC-003);
  - the credit's links point at the licence, the source and the catalog.
- [ ] T043 [P] [US2] Telemetry: `world.created` (contracts/telemetry.md). (waits on 086)
- [X] T044 [US2] `pnpm e2e:worlds`, `pnpm e2e:scenes`, `pnpm e2e:canvas`, then `pnpm e2e:which --diff` and its slices. worlds 24/0, scenes 19/0, canvas 44/0; the slices `e2e:which --diff` names run with T116.
- [X] T045 [US2] Commit: "Spec 088: a new world opens on one of our maps, credited".

**Checkpoint**: SC-003 holds. The walls a base map arrives with already
include the perimeter, once Phase 7's T079 has rebuilt `maps.json`. If US6
lands after US2, T079 re-runs `make base-maps`, and T042 gains its
edge-wall check there.

---

## Phase 4: User Story 3 - Actor view layout (Priority: P2)

**Goal**: the actor view uses the width it has (contracts/layouts.md).

**Independent Test**: `pnpm e2e:actors`, then `pnpm e2e:combat` and
`pnpm e2e:game-systems`.

- [X] T050 [US3] `ActorDetailPage.tsx:387`: replace `Container … max-w-2xl` with the page's own wrapper and the three-column grid of contracts/layouts.md. `max-w-prose` on long text. No change to `Container` or `components/ui/**` (FR-034, FR-036).
- [X] T051 [US3] e2e `apps/web/e2e/layout-widths.spec.ts` (actors slice), actor view part: 375, 1280 and 2560 px; no sideways scroll; 1, 2 and 3 columns; 1800 px ±1 at 2560 (SC-004). Also 320 px and 3840 px for no sideways scroll (FR-035).
- [ ] T052 [US3] `pnpm e2e:actors`, `pnpm e2e:combat`, `pnpm e2e:game-systems`, then `pnpm e2e:which --diff`. So far: actors passed actor-layout-widths (US8 edits in the same run aside); combat and game-systems run with the T116 proof.
- [X] T053 [US3] Commit: "Spec 088: the actor view uses the width it has".

---

## Phase 5: User Story 4 - World page layout (Priority: P2)

**Goal**: the world page uses the width it has.

**Independent Test**: `pnpm e2e:worlds`.

- [X] T055 [US4] `WorldDashboardPage.tsx:135`: the page's own wrapper and the grid of contracts/layouts.md (FR-033, FR-036).
- [X] T056 [US4] The Players card: member count, active link count (for those who run the world), and a link to the players page. It replaces what T019 moved out.
- [X] T057 [US4] `layout-widths.spec.ts`, world page part (worlds slice), as T051.
- [X] T058 [US4] `pnpm e2e:worlds`, then `pnpm e2e:which --diff`. worlds 25/0 (with world-layout-widths), beside instance 45/0 and actors 32/0.
- [X] T059 [US4] Commit: "Spec 088: the world page uses the width it has".

---

## Phase 6: User Story 5 - Clear rolls (Priority: P2)

**Goal**: a GM clears every feed live; nothing comes back; nothing is
deleted.

**Independent Test**: `pnpm e2e:rolls` (both parts).

- [X] T060 [P] [US5] TDD: tests in `rolls/visibility_tests.rs` for `cleared(created_at, rolls_cleared_at)`: never cleared, before, at, and after the clear. See them fail.
- [X] T061 [US5] `cleared` in `rolls/visibility.rs`. T060 passes.
- [X] T062 [US5] Tests in `roll_feed_tests.rs` and `roll_stream_tests.rs`: after a clear, `worldRolls`, `worldRoll`, `worldRollRecords`, the live stream and the catch-up return no cleared roll, for a player and for the GM. A roll made after the clear is returned. A `ROLL_MADE` event recorded before the clear and replayed after it is not delivered (FR-042).
- [X] T063 [US5] Apply `cleared` on every path named in T062. T062 passes.
- [X] T064 [US5] Tests in `mutations_roll_tests.rs`: `clearWorldRolls` by the GM sets `rolls_cleared_at`, records event 39 with `{clearedAt}`, and leaves the `world_roll_records` and `world_attacks` counts unchanged; by a player it is refused; while paused it is refused; for a demoted GM it is refused (FR-040, FR-041).
- [X] T065 [US5] `clearWorldRolls` in `graphql/mutations_roll.rs`, with the `thunderforge.rolls.cleared` counter. T064 passes. The counter waits on 086.
- [X] T066 [US5] `revealRoll` (`mutations_roll.rs:255`) and `rerollRoll` (`mutations_reroll.rs`) refuse a cleared roll with "That roll was cleared.", with tests (FR-043).
- [X] T067 [US5] Web: `engine/world/sync/rolls.ts` handles event 39; `hooks/useWorldRolls.ts` drops every entry at or before `clearedAt` and does not animate it again; tests in `sync/__tests__/rolls.test.ts` (FR-044).
- [X] T068 [P] [US5] Demo, TDD: tests first in the demo's dice and events tests, then `apps/demo/src/backend/handlers/dice.ts` and `events.ts` mirror FR-040 to FR-044 across tabs (FR-046).
- [X] T069 [US5] **Clear rolls** in the feed's header (`components/world/PlayDock/ChatPanel.tsx`), for those who run the world only, with the FR-045 confirmation.
- [X] T070 [US5] e2e `apps/web/e2e/rolls-clear.spec.ts` (rolls slice): a GM and two players roll (one for the GM's eyes, one GM only); the GM clears; every feed empties with no reload; a reload and a reconnect bring nothing back; a player sees no **Clear rolls**.
- [X] T071 [P] [US5] Extend `apps/demo/e2e/rolls-across-tabs.spec.ts` with a clear across two tabs.
- [ ] T072 [P] [US5] Telemetry: `rolls.cleared`. (waits on 086)
- [X] T073 [US5] `pnpm e2e:rolls`, then `pnpm e2e:which --diff` and its slices. rolls 30/0 (web), the demo\'s tests 160/160 and its e2e 3/0, `rolls-across-tabs` included; the rest of `e2e:which --diff` runs in T116.
- [X] T074 [US5] Commit: "Spec 088: the GM clears the roll feed for everyone".

**Checkpoint**: SC-005 holds.

---

## Phase 7: User Story 6 - Edge walls (Priority: P2)

**Goal**: an imported map is walled at its edges, once.

**Independent Test**: `pnpm e2e:canvas`, then `pnpm e2e:scenes`.

- [X] T075 [P] [US6] TDD: `map_import/perimeter_tests.rs` with every fixture in data-model.md (the ambush map's 4 walls, a covered edge, a split edge, a crossing wall, overlapping walls, the 0.5 px tolerance). See them fail.
- [X] T076 [US6] `map_import/perimeter.rs` (new): `perimeter_walls(placement, existing)` (FR-090). T075 passes.
- [X] T077 [US6] Tests in `map_import/tests.rs`: importing the ambush map gives `wallsCreated: 4`, `perimeterWallsCreated: 4`; a re-import leaves 4 edge walls; `wallEdges=false` adds none; a moved perimeter wall survives a re-import; the existing wall, door and light tests are unchanged (FR-091, FR-096).
- [X] T078 [US6] `import_uvtt_impl` (`map_import/mod.rs:115`): the `wallEdges` form field, the replace-then-add in the import's transaction, `perimeter_walls_created` in the `MAP_IMPORTED` payload and the answer, and the `thunderforge.map_import.perimeter_walls` histogram (the histogram waits on 086). T077 passes.
- [X] T079 [US6] `import_offline` (`offline.rs:75`) adds the perimeter, marked `perimeter: true` in `maps.json` (FR-094). Update `offline.rs`'s tests, rebuild the demo's maps and `make base-maps`, and check the demo seed and the base maps carry the walls.
- [X] T080 [US6] Tests in `mutations_levels` tests: `updateSceneLevel` with a new `backgroundAssetId` and `wallEdges` default adds the perimeter for the image's size; a second background moves it; `wallEdges: false` adds none; clearing the background removes none; any other field change leaves walls alone (FR-092).
- [X] T081 [US6] `updateSceneLevel` (`mutations_levels.rs:116`): `wallEdges` on its input, the perimeter replacement in the level's transaction, and `WALL_CHANGED` (10) events for the walls. T080 passes.
- [X] T082 [US6] Web: **Wall the map's edges**, ticked by default, in `MapImportTool.tsx` and in the background picker that sets a plain image; both send `wallEdges` (FR-093).
- [X] T083 [P] [US6] Demo, TDD: `apps/demo/src/backend/mapImport.test.ts` with the same fixtures, then `mapImport.ts` mirrors FR-090 and FR-091 (FR-095).
- [X] T084 [US6] e2e `apps/web/e2e/canvas-map-edge-walls.spec.ts` (canvas slice; add to `slices.json` if needed): import `grassy-path-ambush.dd2vtt` with the defaults; the scene has 4 walls on its edges; a token dragged 10 cells past an edge ends inside the map; a re-import leaves 4; with the box unticked, none (SC-008). canvas 46/0 (the move off the map through `moveOwnToken`, as `drawn-walls-block.spec.ts` sends its moves).
- [X] T085 [P] [US6] If US2 has landed, add to `world-base-map.spec.ts`: a world created on the default map has its 4 edge walls. worlds 24/0.
- [ ] T086 [P] [US6] Telemetry: `map.imported`. (waits on 086)
- [X] T087 [US6] `pnpm e2e:canvas`, `pnpm e2e:scenes`, the demo's tests, then `pnpm e2e:which --diff` and its slices. So far: canvas 46/0, scenes 19/0, worlds 24/0, the demo's tests 160/160; rolls 30/0 with US5, whose demo `rolls-across-tabs` (3/0) passed on the re-run after failing once (a board never played the die).
- [X] T088 [US6] Commit: "Spec 088: an imported map is walled at its edges".

**Checkpoint**: SC-008 holds.

---

## Phase 8: User Story 7 - Mail form (Priority: P3)

**Goal**: one Save that sends only what changed, and a warning before
losing changes.

**Independent Test**: `pnpm e2e:instance`.

- [X] T090 [P] [US7] TDD: `apps/web/src/pages/admin/settingsForm.test.ts` for every function in data-model.md: trimming, the secret rule, fixed keys, form order with `mail.enabled` last, partial results, and rebase. See them fail.
- [X] T091 [US7] `apps/web/src/pages/admin/settingsForm.ts`. T090 passes (FR-050, FR-051).
- [X] T092 [P] [US7] `apps/web/src/hooks/useUnsavedChanges.ts`, with tests for `beforeunload` and the link-click guard (FR-054).
- [X] T093 [US7] `MailPanel.tsx`: one form over the model, one Save and one Discard, "N unsaved changes", "k of n saved", the guard on, and the settings page's section switches routed through it (FR-052, FR-053, FR-054).
- [X] T094 [US7] `SettingRow` (`InstanceSettingsPanel.tsx:163-339`): follow the `setting` prop while clean (FR-055).
- [X] T095 [US7] e2e `apps/web/e2e/instance-mail-form.spec.ts` (instance slice): **Save** disabled when clean; change 2 keys and count exactly 2 `updateInstanceSetting` requests by interception (SC-006); the section-switch guard; a refused key stays dirty with its error.
- [X] T096 [P] [US7] Adjust `instance-mail.spec.ts` and `mail-delivery.spec.ts` where they press a per-row Save. instance-mail.spec.ts pressed no per-row Save; only mail-delivery.spec.ts changed.
- [ ] T097 [P] [US7] Telemetry: `settings.saved`, `settings.unsaved_warning`. (waits on 086)
- [X] T098 [US7] `pnpm e2e:instance`, then `pnpm e2e:which --diff` and its slices. instance 45/0 (with instance-mail-form and the reworked mail-delivery).
- [X] T099 [US7] Commit: "Spec 088: the mail settings save only what changed".

---

## Phase 9: User Story 8 - Hero polish (Priority: P3)

**Goal**: a player's own sheet and look are a tap away on a phone.

**Depends on**: `hotfix-player-hero-edit` (all).

**Independent Test**: `pnpm e2e:actors`, `pnpm e2e:hero-builder`.

- [X] T100 [US8] `PlayersPage.tsx`: at under 640 px the viewer's card first; **Open sheet** and **Edit look** full width and at least 44 px tall (FR-060).
- [X] T101 [US8] "Ask your GM for a character" on a player's card with no claimed character (FR-063).
- [X] T102 [US8] The hero builder dialog full screen at under 640 px, closing back to where it opened (FR-061). No code change: the builder dialog was already full screen at under 640 px; T104 proves it at 375x812.
- [X] T103 [US8] `?from=players` on the links from the card; `ActorDetailPage` sends its back control to the players page when set (FR-062).
- [X] T104 [US8] Extend `players-hero-edit.spec.ts` (from the hotfix) at 375 px: card order, control sizes, a full-screen builder, the back control, and the empty state.
- [ ] T105 [P] [US8] Telemetry: `hero.opened_from_players`. (waits on 086)
- [X] T106 [US8] `pnpm e2e:actors`, `pnpm e2e:hero-builder`, then `pnpm e2e:which --diff`. actors 32/0; hero-builder 18/1 (standalone 7/0). The one failure is Quick NPC (hero-builder-npc.spec.ts:468: the preview does not change after Open in builder, then Use), and it fails again alone (6/1). Nothing 088 changes is on that path (QuickNpcDialog, apps/hero-builder and packages/heroes are untouched), so it is not this story.
- [X] T107 [US8] Commit: "Spec 088: a player opens their own sheet and look from the players screen".

---

## Phase 10: Docs and proof

- [X] T110 [P] `docs/guides/inviting-players.md` (new): world links, the optional use limit and when a use counts, expiry, revoking, and why a new person needs an instance invitation (FR-080).
- [X] T111 [P] `docs/guides/your-first-world.md` (new): the starting map, **None**, and the credit.
- [X] T112 [P] `docs/guides/rolls.md`: clearing, and what clearing keeps.
- [X] T113 [P] `docs/guides/doors-and-walls.md`: the edge walls, the box, and how to remove them.
- [X] T114 [P] `docs/INSTANCE_CONFIGURATION.md` and `.env.example`: `THUNDERFORGE_BASE_MAPS_DIR` (FR-081).
- [X] T115 [P] `docs/CONTRIBUTING.md`: `useUnsavedChanges`, the settings form model, the clear rule beside the visibility rule, and the perimeter mark.
- [ ] T116 The Proof run (spec.md, Proof): `make lint`, `cargo test -p thunderforge-server`, the web's tests and typecheck, the demo's tests, every slice listed, then `pnpm e2e:which --diff` and each slice it names. Record the results in research.md under `## Proof` (SC-007).
- [ ] T117 Mark spec.md's status "Implemented", and commit: "Spec 088: proven by its slices".
- [ ] T118 `mcp__gitops__merge_ff_only` onto main. If it refuses, stop and ask the owner.

---

## Dependencies & Execution Order

- **Phase 0 → Phase 1 → stories.** Every story needs Phase 1.
- **US1** needs `hotfix-world-permissions`. T028 also needs
  `hotfix-invite-uses`. T019 must come before T056 (US4's Players card).
- **US2** stands alone. If US6 lands first, T037 picks up the perimeter for
  free; if US2 lands first, T079 and T085 add it.
- **US3 and US4** stand alone, and touch different pages.
- **US5** stands alone.
- **US6** stands alone. T079 rebuilds what US2 serves.
- **US7** stands alone.
- **US8** needs `hotfix-player-hero-edit`. T103 touches `ActorDetailPage`
  after US3's T050, so US8 follows US3.
- **Phase 10** after every story it documents.

## Parallel Opportunities

- T005, T006 and T007.
- Once Phase 1 is done, US2, US3, US5, US6 and US7 can run side by side.
  They share no files, apart from `schema.graphql`, which each regenerates.
  Regenerate it again after each rebase.
- Inside a story, the `[P]` tasks: the TDD test files, telemetry, and the
  demo mirrors.
- e2e runs share the external stack and its lock. Run slices one at a
  time, with `--workers=1`.

## Implementation Strategy

1. **The P1 pair first**: US1 (as soon as `hotfix-world-permissions` is
   on main) and US2. Those are the two things the table hit first.
2. **Then the P2 stories**: US5 and US6 (play at the table), then US3 and
   US4 (layout).
3. **Then the P3 stories**: US7, and US8 once its hotfix is in.
4. Each story is committed and proven by its own slice before the next
   starts, so any one can ship or be reverted alone.

## Task count per phase

| Phase | Tasks |
| --- | --- |
| 0 Setup | 3 |
| 1 Foundational | 5 |
| 2 US1 World links | 20 |
| 3 US2 Base maps | 16 |
| 4 US3 Actor view | 4 |
| 5 US4 World page | 5 |
| 6 US5 Clear rolls | 15 |
| 7 US6 Edge walls | 14 |
| 8 US7 Mail form | 10 |
| 9 US8 Hero polish | 8 |
| 10 Docs and proof | 9 |
| **Total** | **109** |
