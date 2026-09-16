---
description: "Task list for spec 044, the hero builder"
---

# Tasks: The Hero Builder

**Input**: Design documents from `/specs/044-hero-builder/`

**Prerequisites**: spec.md (decided 2026-09-14), research.md, plan.md,
data-model.md, contracts/builder.md, quickstart.md

**Tests**: Required. Each plan phase ends with its e2e run and the log searched
for `✘`. Contract rules B1–B9 (contracts/builder.md §5) each get a test:
B1–B3 in the phase (a) suite, B4–B5 in the phase (b) e2e, B6–B7 and B9 in
`cargo test`, B8 in both.

**No playtest.** No playtest scenario builds a hero, and writing one to satisfy
a habit would prove nothing the e2e does not. The acceptance proof is the four
Playwright specs named below.

**Proof per phase**: phase (a) runs
`pnpm -F @thunderforge/heroes check` and
`pnpm -F @thunderforge/hero-builder-app test:e2e`, neither of which needs a
stack. Phases (b) to (d) run
`node scripts/e2e-parallel.mjs --shards=1 --only=<specs>` with no other e2e or
playtest running, and the log searched for `✘`. `pnpm verify` does **not**
type-check the web app, so every web phase also runs
`pnpm -F @thunderforge/web exec tsc --noEmit`.

**Phases map to the plan's letters**: tasks Phase 2 and Phase 3 are plan phase
**(a)**, Phase 4 is **(b)**, Phase 5 is **(c)**, Phase 6 is **(d)**, and Phase 7
is polish. Research, data-model, contracts and quickstart all use the letters.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1–US7 from spec.md

---

## Phase 1: Setup

**Purpose**: the working set, and the numbers the later phases are measured against.

- [ ] T001 Read `specs/044-hero-builder/contracts/builder.md`, `research.md` and `plan.md` into the working set; every task below is measured against contract rules B1–B9 and the five findings in plan.md's "What the code says, against what the spec assumed"
- [ ] T002 [P] Record today's catalogue in this file's Notes section as the baseline the run-time-derived e2e is sanity-checked against once: 12 parts (`packages/heroes/src/spec.ts:91-104`), 12 colour fields (`:128-141`), 2 flags (`:144`), 6 sizes (`:114-123`), 8 skin tones, 12 monster tones
- [ ] T003 [P] Confirm `pnpm-workspace.yaml`'s `apps/*` and `packages/*` globs already admit `apps/hero-builder` and `packages/hero-builder`, so no workspace file changes; note it here

---

## Phase 2: (a) The catalogue tells the builder everything (Priority: P1) — US1, US2

**Goal**: `packages/heroes` exports labels, palettes, a seeded randomiser, a minimal-spec function and a preset writer, so the builder holds no list of its own and a missing label fails the package rather than a screen three layers away.

**Independent test**: with no builder in existence, `pnpm -F @thunderforge/heroes check` passes; deleting one label from `HERO_LABELS` makes it fail by naming the field.

- [ ] T004 [P] [US2] Add `HERO_LABELS` in new `packages/heroes/src/labels.ts` per contracts §1: a label for every key of `HERO_PARTS`, `HERO_COLORS` and `HERO_FLAGS`, for `name`, `title` and `size`, and for every choice of every part and every size category
- [ ] T005 [P] [US1] Add `HERO_PALETTES` in new `packages/heroes/src/palettes.ts`: `skin` → `SKIN_TONES`' values, `hideColor` → `MONSTER_TONES`' values, and a curated `#rrggbb` list for the other ten colour fields; every entry must satisfy `HEX_COLOR` (`packages/heroes/src/color.ts:4`)
- [ ] T006 [US1] Add `randomHero(seed, locked?)` in new `packages/heroes/src/random.ts`, built on the existing `seeded()` chooser (`packages/heroes/src/seed.ts:56`): choices only from `HERO_PARTS`, colours only from `HERO_PALETTES`, a locked field omitted from the result so the caller's value survives a merge (FR-007)
- [ ] T007 [US1] Add `minimalSpec(spec)` in new `packages/heroes/src/minimal.ts`: drop a field exactly when removing it leaves `resolveHero()` unchanged, so the six derived colours (`packages/heroes/src/spec.ts:319-335`) keep following; `name` is always kept (FR-009)
- [ ] T008 [US1] Add `presetSource(slug, spec)` in new `packages/heroes/src/presetSource.ts`: a `HeroPreset` entry as TypeScript source, writing `SKIN_TONES.<name>` where the skin matches a named tone (FR-014)
- [ ] T009 [US1] Re-export T004–T008 from `packages/heroes/src/index.ts` beside the existing exports; add none to `package.json` (the package stays dependency-free)
- [ ] T010 [P] [US2] Test in `packages/heroes/src/labels.test.ts`: every field and every choice in the catalogue has a label, and a deliberately removed label fails by naming the field (US2 scenario 3)
- [ ] T011 [P] [US1] Test in `packages/heroes/src/random.test.ts`: every `randomHero` result passes `validateHero`; one seed always gives one hero; a locked field is absent from the result; no colour is produced that is not in `HERO_PALETTES`
- [ ] T012 [P] [US1] Test in `packages/heroes/src/minimal.test.ts`: `resolveHero(minimalSpec(s))` equals `resolveHero(s)` for all twelve presets and for a hundred seeded random heroes; a set field equal to its derived default is dropped; `presetSource` output names a `SKIN_TONES` entry where one matches
- [ ] T013 [US1] Prove: `pnpm -F @thunderforge/heroes check` and `pnpm verify` (whose `heroes` step is that command, `scripts/verify.mjs:144-147`); record results here

**Checkpoint**: everything a builder needs to draw its own controls exists, and nothing renders it yet.

---

## Phase 3: (a) A hero we can look at while we change it (Priority: P1) — US1, US2

**Goal**: `packages/hero-builder` renders the whole catalogue from data, and `apps/hero-builder` hosts it with no server, no auth and no GraphQL.

**Independent test**: with only the standalone app running, load a preset, change one choice of every part, export, import the export back, and confirm it draws exactly what was on screen.

- [ ] T014 [US1] Create `packages/hero-builder/package.json` (`@thunderforge/hero-builder`, private, `type: module`, `"." → ./src/index.ts` as `packages/heroes` does), `tsconfig.json` and `src/index.ts`; React 19 and `react-dom` as **peer** dependencies only, `@thunderforge/heroes` as a workspace dependency, and nothing imported from `apps/web`
- [ ] T015 [US1] `packages/hero-builder/src/preview/HeroPreview.tsx`: portrait and token for a spec, each SVG given an id prefix derived from the caller's `idPrefix` so no two elements in the host document share an id (FR-011, B2)
- [ ] T016 [US1] `packages/hero-builder/src/controls/`: one control kind per field kind, all generated from `HERO_PARTS`, `HERO_COLORS`, `HERO_FLAGS`, `SIZES` and `HERO_LABELS` — a part is one arrow-key group (FR-017), a colour is `HERO_PALETTES` swatches plus a `#rrggbb` field whose hex is readable as text (FR-008, FR-018), a flag is a switch, plus name, title and size. No part, choice or colour key appears in this directory's source (B1)
- [ ] T017 [US1] Set-versus-following in `packages/hero-builder/src/controls/`: a field the user has not set shows its resolved value as "following", and a set field can be returned to following; decided by comparing `resolveHero(spec)` with `resolveHero(minimalSpec(spec))`, never by a list of derived fields (FR-005)
- [ ] T018 [US1] `packages/hero-builder/src/HeroBuilder.tsx`: state is a `HeroSpec`, every spec passes `validateHero` before anything is drawn, an invalid one is reported by field through `onInvalid` and changes nothing on screen (FR-004, FR-010, B3); continuous input redraws at most once per animation frame (FR-021); the props are exactly contracts §2
- [ ] T019 [US1] `packages/hero-builder/src/io/`: export the portrait SVG, the token SVG and `minimalSpec` as JSON, each a file the user saves (FR-013); import a spec from pasted text or a file, refused whole with every problem named (FR-010); "copy as preset" puts `presetSource` output on the clipboard (FR-014). No SVG import exists anywhere in this directory (FR-026)
- [ ] T020 [US1] Create `apps/hero-builder`: `package.json` (`@thunderforge/hero-builder-app`, scripts `dev`, `build`, `preview`, `test:e2e`), `vite.config.mts` on the web app's Vite with a port of its own, `index.html`, `src/main.tsx`. No server, no auth, no GraphQL, no `@/…` alias into `apps/web` (FR-012)
- [ ] T021 [US1] The standalone app's chrome in `apps/hero-builder/src/App.tsx`: a preset picker over `PRESET_HEROES` (FR-006), randomise with the seed shown and re-enterable and per-field locks (FR-007), and the export/copy actions passed to `HeroBuilder` through `actions`
- [ ] T022 [US1] `apps/hero-builder/playwright.config.ts`: `webServer` runs this app's own `preview`, one worker, no global setup, no database — and a note saying why this suite is not in `scripts/e2e-parallel.mjs` (its two suites are both rooted at `apps/web`, `scripts/e2e-parallel.mjs:185-193`, and every lane stands up a stack this app does not use)
- [ ] T023 [US1] [US2] `apps/hero-builder/e2e/builder.spec.ts` part 1: derive the expected controls from `HERO_PARTS`, `HERO_COLORS`, `HERO_FLAGS` and `SIZES` **at run time** and assert one labelled, selectable group per field, with no count written in the spec file (FR-001, FR-002, SC-002)
- [ ] T024 [P] [US1] `builder.spec.ts` part 2: every control change redraws both previews with no reload (FR-003), and the change is reflected within 100 ms measured in the page and logged per run (SC-006)
- [ ] T025 [P] [US1] `builder.spec.ts` part 3: every preset loads and draws exactly what `packages/heroes/src/cli.ts` writes for that preset (US1 scenario 3); export then import reproduces the on-screen hero byte for byte for every preset (SC-003)
- [ ] T026 [P] [US1] `builder.spec.ts` part 4: no duplicated element id on the page after loading every preset (SC-005, B2); an invalid pasted spec lists every problem by field and changes nothing (FR-010)
- [ ] T027 [P] [US1] `builder.spec.ts` part 5: the whole builder walked by keyboard alone, every choice and swatch with an accessible name and its hex readable as text (SC-004, FR-017, FR-018); at 375 px both previews stay visible, nothing scrolls sideways, every target is at least 44 px (FR-020)
- [ ] T028 [US1] `builder.spec.ts` part 6: randomise is reproducible from the shown seed and leaves a locked part unchanged (FR-007); "copy as preset" produces an entry that, pasted into `presets.ts`, passes `pnpm -F @thunderforge/heroes check` and draws identically (FR-014, SC-001)
- [ ] T029 [US2] The SC-002 proof, by hand and once: on a throwaway branch add a choice to one list in `HERO_PARTS` with its label, run T013's and T031's commands unchanged, confirm the e2e finds, selects and draws it with **no** edit to `packages/hero-builder` or to `builder.spec.ts`; record the result here and discard the branch
- [ ] T030 [P] [US1] `packages/hero-builder/README.md` and `apps/hero-builder/README.md`: what each is, how to run the app, and that the library takes React from its host — in the manner of `apps/engine-sandbox/README.md`
- [ ] T031 [US1] Prove: `pnpm -F @thunderforge/hero-builder-app test:e2e`, `pnpm -F @thunderforge/hero-builder exec tsc --noEmit`, `pnpm -F @thunderforge/heroes check` and `pnpm verify`; record results and the measured SC-006 figure here

**Checkpoint**: SC-001 — a new preset reaches `presets.ts` in under five minutes without writing code. The owner's first stated use is done.

---

## Phase 4: (b) A Game Master gives an NPC a face (Priority: P1) — US3, US4

**Goal**: "Build a hero" inside the imagery panel, so it appears on every page that mounts the panel, and Quick NPC in the compendium. **This is the phase that answers the owner's request of 2026-09-15.**

**Independent test**: as a Game Master, open an NPC's edit page, build a hero, save, and confirm the portrait and token are stored and served as WebP and shown in the panel; then make three NPCs with Quick NPC and confirm three different faces.

- [ ] T032 [US3] Add `@thunderforge/hero-builder` and `@thunderforge/heroes` as workspace dependencies of `apps/web`, and whatever `apps/web/vite.config.mts` needs to resolve them from source; the library must take the app's React, so check that the existing force-resolution block (`apps/web/vite.config.mts:41-47`, written for `@thunderforge/genie`) is not defeated and that no second React reaches the page
- [ ] T033 [US3] `apps/web/src/pages/world/actor/HeroBuilderDialog.tsx`: `import()`s `@thunderforge/hero-builder` at open time and nowhere else (FR-022), moves focus into the dialog, keeps it there and returns it to the control that opened it (FR-019), and passes "Save to this character" as the builder's `actions`
- [ ] T034 [US3] Add the "Build a hero" control (`data-testid="actor-imagery-build"`) to `apps/web/src/pages/world/actor/ActorImageryPanel.tsx`, rendered exactly when `canEdit` — the condition that already governs the file inputs (`:175`) — and opening the builder with the actor's label as the hero's name (FR-023)
- [ ] T035 [US3] The save flow in `ActorImageryPanel.tsx`: where either role already has an image, say before starting that both will be replaced (FR-025); upload portrait then token through the existing `uploadActorImage` (`apps/web/src/api/actors.ts:390-414`) as `image/svg+xml` files built from the SVG strings; report each role separately, claim success for neither unless its row came back, and offer a failed role a retry of its own (FR-024, FR-025, B4)
- [ ] T036 [P] [US3] A single helper in `apps/web/src/pages/world/actor/heroFiles.ts` turning an SVG string and a role into a `File`; no new mutation, no new endpoint, no browser rasterisation (FR-024, B4)
- [ ] T037 [US4] `apps/web/src/pages/world/compendium/QuickNpcDialog.tsx`: a name field, the previews of a `randomHero`, a reroll, **a reserved space below the name for a later system NPC template** (FR-027, Decisions 2), "Open in builder" handing the current hero to `HeroBuilderDialog`, and Create
- [ ] T038 [US4] Add the "Quick NPC" control (`data-testid="quick-npc"`) beside the existing Add-NPC link in `apps/web/src/pages/world/compendium/NpcCompendiumTab.tsx:261-270`, on the same condition, so a Player is offered none
- [ ] T039 [US4] Quick NPC's create path: `createActor({ worldId, label, isNpc: true })` then the same two uploads; the Game Master stays in the compendium with the new NPC selected (US4 scenario 2); where an upload fails the NPC **remains**, is reported as lacking art and offers a way to add it, and the dialog reports no success (FR-028)
- [ ] T040 [P] [US4] Show an NPC with no token or portrait as lacking art in `NpcCompendiumTab.tsx`, using the imagery it already fetches (`:64-67`), with an accessible label and a link to its edit page
- [ ] T041 [P] [US3] e2e `apps/web/e2e/hero-builder-npc.spec.ts` part 1: a Game Master builds and saves to an existing NPC; both roles appear in the panel; the served bytes are WebP (`RIFF`/`WEBP`), asserted the way the P8 spec does (`apps/web/e2e/world-compendium.spec.ts:268`); the builder opened with the NPC's name (SC-007)
- [ ] T042 [P] [US3] `hero-builder-npc.spec.ts` part 2: an NPC that already has art warns before saving that both roles will be replaced; one role forced to fail reports that role failed and the other saved, and the failed role retries alone (FR-025); closing without saving leaves the images unchanged
- [ ] T043 [P] [US3] `hero-builder-npc.spec.ts` part 3: a Viewer on the same page is offered no build or save control **and** a direct `uploadActorImage` call is refused (US3 scenario 5, B5)
- [ ] T044 [P] [US4] `hero-builder-npc.spec.ts` part 4: three NPCs from an empty compendium through Quick NPC, each named with a portrait and a token and all three faces different (SC-008); reroll changes the face; "Open in builder" carries the current hero; a Player is offered no Quick NPC; a failed upload leaves the NPC listed as lacking art (FR-028)
- [ ] T045 [US3] `hero-builder-npc.spec.ts` part 5, the two rules a builder must not weaken: a built NPC placed twice has both tokens wearing the face, because a copy reads its actor's art (`src/server/src/graphql/token_art.rs:41-58`, ADR-102); and a hidden NPC's built portrait is refused to a player while its token art is still served on a scene they can see (`src/server/src/assets_serve/actor.rs:155-165`)
- [ ] T046 [P] [US3] Prove SC-012 and FR-022 from the built chunk graph: `pnpm -F @thunderforge/web build`, then show the builder's code is in no chunk loaded by a page that does not open it; record the figures here
- [ ] T047 [US3] Prove: `node scripts/e2e-parallel.mjs --shards=1 --only=hero-builder-npc,world-compendium` with the log searched for `✘`; `pnpm -F @thunderforge/web exec tsc --noEmit`; `pnpm verify`; record results and the SC-007/SC-008 timings here

**Checkpoint**: the owner's 2026-09-15 request is answered — a "Build a hero" control sits beside the portrait and token on an actor's edit page, and a named NPC with a face takes three interactions.

---

## Phase 5: (c) A player's own character (Priority: P2) — US5

**Goal**: the player who holds a character may change its art and nothing else; a Game Master may turn that off for a world or lock one character; a Game Master's own authority is untouched.

**Independent test**: as an invited player, create your own character with a built hero; confirm a second player cannot change it, that the world setting off refuses it, that a lock refuses that one character only, and that the Game Master can still replace the art.

- [ ] T048 [US5] Write `docs/adrs/<date>-104-a_characters_look_belongs_to_whoever_holds_it.md` (PROPOSED) from plan.md's Constitution Check and contracts §5 B6: why a claim confers a right that ADR-050's ladder does not, why the right is imagery-only rather than an Editor grant row, and why the two withdrawals are a world setting and a per-character lock rather than an approval queue (FR-030c); add the row to `docs/adrs/README.md`. Take the next free number at the time of writing — 104 is free as of 2026-09-15, and specs 052–055 are in flight
- [ ] T049 [US5] Migration `src/server/migrations/<date>-044-player-actor-art/` (up/down) per data-model.md: `worlds.allow_player_actor_art BOOLEAN NOT NULL DEFAULT true` backfilled `true` for every existing world, and `world_actors.art_locked BOOLEAN NOT NULL DEFAULT false`; regenerate `src/server/src/schema.rs`
- [ ] T050 [US5] Set the hand-kept creation defaults to match: `allow_player_actor_art: true` in `src/server/src/graphql/mutations_worlds.rs:141-145` and in the adapters, so a new world agrees with every old one (data-model.md notes why this default is the opposite of its three neighbours)
- [ ] T051 [US5] New `src/server/src/auth/actor_imagery.rs`: `may_change_actor_imagery(conn, user, actor_id)` implementing B6 — Editor or above by the existing ladder, **or** a live claim in `world_actor_claims` on an actor whose world has `allow_player_actor_art = true` and whose `art_locked` is false; returns the three distinct refusals named in contracts §5. Register the module in `src/server/src/auth/mod.rs`
- [ ] T052 [US5] Call it in place of `require_actor_permission(Editor)` in both `upload_actor_image_impl` and `remove_actor_image_impl` in `src/server/src/graphql/mutations_actor_images.rs:113-121` and `:212-247`; the pause gate (`:122-124`) stays exactly as it is, and no other actor mutation changes
- [ ] T053 [P] [US5] `updateWorldAllowPlayerActorArt` in `src/server/src/graphql/mutations_worlds.rs`, following `update_world_auto_apply_npc_damage_impl` (`src/server/src/graphql/mutations_attacks.rs:106-130`): `is_dm_of_world`, `refuse_if_paused` inside the blocking closure, one `diesel::update` returning `GraphQLWorld` (FR-030a)
- [ ] T054 [P] [US5] `setActorArtLocked(actorId, locked)` in `src/server/src/graphql/mutations_actors.rs`, Game Master only, beside the existing `setActorUnique`/visibility setters; locking changes no image (FR-030b)
- [ ] T055 [US5] Expose `allowPlayerActorArt` on `GraphQLWorld` and `artLocked` on `GraphQLWorldActor` (`src/server/src/graphql/types_scene.rs:162-247`), and add both to `play_pause_surface_tables.rs` where the other two setters are listed
- [ ] T056 [P] [US5] `cargo test` in new `src/server/src/auth/actor_imagery_tests.rs`, one per clause of B6: a claimant may write imagery and a non-claimant may not; a player who created their own character may, and stops when the claim ends; the grant reaches no other actor mutation; the world setting off refuses every actor in that world; a lock refuses that actor whatever the setting; a Game Master is refused by neither and may replace a player's art; the three refusals carry three different messages
- [ ] T057 [US5] Regenerate `src/app/schema.graphql` (`node scripts/check-graphql-contract.mjs --schema --fix`); `pnpm verify` green including `graphql-schema` and `graphql-ops`
- [ ] T058 [P] [US5] Add `updateWorldAllowPlayerActorArt` and `setActorArtLocked` to `apps/web/src/api/world.ts` and `apps/web/src/api/actors.ts`, and carry `allowPlayerActorArt` / `artLocked` on the records the pages already read
- [ ] T059 [US5] Mount `ActorImageryPanel` on `apps/web/src/pages/world/actor/ActorDetailPage.tsx` with `canEdit` from the new rule, so the panel and "Build a hero" reach a player's own character (FR-033). If the separate in-flight change has already mounted it, this task is checking `canEdit` and saying so
- [ ] T060 [P] [US5] The Game Master's lock control on `ActorDetailPage.tsx`, beside the existing unique and visible-to-players toggles, with an accessible label saying it locks the look and not the character (FR-030b)
- [ ] T061 [P] [US5] The world setting "Players may change their character's art" wherever the world's other Game-Master settings live under `apps/web/src/pages/world/`, defaulting on and explaining what it withdraws (FR-030a)
- [ ] T062 [US5] Let "Create your own character" build a hero before creating in `apps/web/src/pages/world/ActorSelectionPage.tsx:101-124`: create through `createAndClaimActor` first, upload second, and on a failed upload leave the character without art and say where to add it (FR-034, FR-028)
- [ ] T063 [P] [US5] e2e `apps/web/e2e/hero-builder-player.spec.ts` covering the independent test, every refusal attempted **both** by looking for the control and by calling the mutation directly (SC-010): a second player refused, the world setting off refused, one character locked and another of the same player's unaffected, the claim released and then refused, and the Game Master replacing the art regardless
- [ ] T064 [US5] Prove: `node scripts/e2e-parallel.mjs --shards=1 --only=hero-builder-player,actor-claims,world-actor-permissions` with the log searched for `✘`; `cargo test -p thunderforge-server --lib`; `tsc --noEmit`; `pnpm verify`; record results here

**Checkpoint**: a player gives their own character a face, and a Game Master has two ways to stop them and one way to overrule them.

---

## Phase 6: (d) A saved hero re-opens as a hero (Priority: P2/P3) — US6, US7

**Goal**: the spec that drew an image is stored with that image, travels with a collection copy and a personal export, and re-opens in the builder with every choice intact.

**Independent test**: build and save a hero for an NPC, reload, re-open the builder and confirm every control shows the saved choice; then copy the NPC through a collection into a second world and re-open it there.

- [ ] T065 [US6] Write `docs/adrs/<date>-105-a_stored_image_remembers_the_spec_that_drew_it.md` (PROPOSED) from research R4 and data-model.md, recorded as an extension of ADR-057: why the spec hangs on the image row and not on the actor, the three rejected alternatives, the duplication accepted, the server-side schema check, and the append-only consequence for the catalogue (FR-038); add the row to `docs/adrs/README.md`. Take the next free number at the time of writing
- [ ] T066 [P] [US6] Add `HERO_SPEC_SCHEMA` (JSON Schema, draft 2020-12) in new `packages/heroes/src/schema.ts`, derived from `HERO_PARTS`, `HERO_COLORS`, `HERO_FLAGS`, `SIZES` and the text bounds `validateHero` applies; export it from `index.ts`
- [ ] T067 [P] [US6] Test in `packages/heroes/src/schema.test.ts`: the schema and the lists agree (a choice added to any list and absent from the schema fails); every preset validates against it; each case `validateHero` refuses is refused by the schema too
- [ ] T068 [US6] Migration `src/server/migrations/<date>-044-image-hero-spec/` (up/down): `world_actor_images.hero_spec JSONB NULL`; regenerate `src/server/src/schema.rs` and the models (`src/server/src/models.rs:2056-2078`)
- [ ] T069 [US6] New `src/server/src/heroes/spec_schema.rs` implementing B7: refuse anything that is not a JSON object, anything over 4 KB serialised, and anything failing `HERO_SPEC_SCHEMA` (vendored into the crate, or read from the package at build time — whichever a test can keep honest); a refusal names what was wrong and stores **neither** the spec nor the image
- [ ] T070 [US6] Add the optional `heroSpec: JSON` argument to `uploadActorImage` and write it in the same upsert that stores the image in `src/server/src/graphql/mutations_actor_images.rs:101-178`; an upload with no argument writes `NULL`, so replacing a built image with a file clears that role's spec (FR-035, B8)
- [ ] T071 [P] [US6] Expose `heroSpec` on `GraphQLActorImage` (`src/server/src/graphql/types_actors.rs:78-98`) and return it from `actor_images_impl` (`:185-204`)
- [ ] T072 [P] [US6] `cargo test` in new `src/server/src/heroes/spec_schema_tests.rs` for B7: an array refused; an over-cap spec refused; an unknown field refused; a valid spec stored; a refusal leaving no image row written
- [ ] T073 [US7] Carry `hero_spec` through the collection copy in `src/server/src/collections/copy.rs:597-616` — the select and the insert both name columns, so it does **not** travel by itself, whatever research R4 says (FR-039)
- [ ] T074 [P] [US6] Carry `hero_spec` in the personal export: `ExportedImage` and its query in `src/server/src/users/export_content.rs:304-314` (FR-040)
- [ ] T075 [P] [US7] `cargo test` for B9 beside the existing copy tests (`src/server/src/collections/copy_asset_and_link_tests.rs`): a copied actor's images arrive with their specs, saving in the new world changes the copy only, and the export carries the spec
- [ ] T076 [P] [US6] Make the catalogue append-only now that specs are stored (FR-038): a test in `packages/heroes` over a recorded list of every choice ever shipped, failing when one is renamed or removed while allowing a new one and allowing a choice to change how it draws
- [ ] T077 [US6] Open the builder from the stored spec in `apps/web/src/pages/world/actor/HeroBuilderDialog.tsx`, preferring the portrait's; where the two roles' specs differ, or only one role has one, say so; where neither has one, start from the actor's name as in phase (b) (FR-036)
- [ ] T078 [US6] Send `heroSpec` with each role's upload from the save flow in `ActorImageryPanel.tsx`, and send none from the plain file input so an uploaded file clears that role's spec (FR-035, B8)
- [ ] T079 [P] [US6] A stored spec that no longer validates opens with every problem named by field and the stored images untouched — nothing silently redrawn from defaults (US6 scenario 4); every client runs `validateHero` before drawing a stored spec (FR-037)
- [ ] T080 [US6] Regenerate `src/app/schema.graphql`; `pnpm verify` green including `graphql-ops`
- [ ] T081 [US6] [US7] e2e `apps/web/e2e/hero-builder-saved.spec.ts` covering the independent test plus: a token replaced by an uploaded file re-opens from the portrait's spec and says the token is no longer a built hero; an actor with no stored spec opens from its name; a hand-invalidated spec shows its problems and leaves the images alone (SC-011)
- [ ] T082 [US6] Prove: `node scripts/e2e-parallel.mjs --shards=1 --only=hero-builder-saved,collections-copy` with the log searched for `✘`; `cargo test -p thunderforge-server --lib`; `pnpm -F @thunderforge/heroes check`; `tsc --noEmit`; `pnpm verify`; record results here

**Checkpoint**: every story in the spec is real, and a hero travels with the actor that wears it.

---

## Phase 7: Polish & cross-cutting

- [ ] T083 [P] Accept both ADRs once their phases are proven, dating the acceptance and naming the tasks that proved them, as ADR-101 and ADR-102 do
- [ ] T084 [P] Document the standalone builder where a developer will find it: `apps/hero-builder/README.md` linked from wherever `apps/engine-sandbox` is linked, and a line in the repository's development docs saying it needs no stack
- [ ] T085 [P] Re-read the spec's Assumptions against what shipped and correct anything that moved, in the manner of plan.md's findings section; in particular the catalogue's size, which the spec states as six parts and ten colours and which has been twelve and twelve since 2026-09-12
- [ ] T086 Full run: `node scripts/e2e-parallel.mjs --shards=2` with nothing else running; search the log for `✘`; record the time and result beside the run recorded in spec 046's T109
- [ ] T087 `pnpm verify`, `pnpm -F @thunderforge/web exec tsc --noEmit` and `make lint-wasm` before the final commit of the feature

---

## Dependencies

```text
Phase 1 (setup)
  └─ Phase 2 (a: the catalogue)          ── blocks everything below
       └─ Phase 3 (a: the builder)       ── ships first; SC-001
            └─ Phase 4 (b: a GM's NPC)   ── the owner's 2026-09-15 request
                 ├─ Phase 5 (c: a player) ── needs 4's control; adds authorisation
                 └─ Phase 6 (d: saved specs) ── needs 4 (something to save);
                                                independent of 5
Phase 7 (polish) ── after every phase that ships
```

Phases 5 and 6 are independent of each other and may ship in either order. Both
change the data model, and each carries its own ADR.

Within a phase: package additions → library → host → migration → server → GraphQL
→ tests → web → e2e → proof.

## Parallel opportunities

- **Phase 2**: T004 and T005 are separate new files; T010, T011 and T012 are
  separate test files and may be written alongside T006–T008.
- **Phase 3**: T024–T027 are separate cases in one spec file and may be written
  alongside each other once T023 has established how the catalogue is derived;
  T030 (the READMEs) any time after T020.
- **Phase 4**: T036 and T040 alongside T033–T035; T041–T044 split across
  separate `test.describe` blocks; T046 needs only a build.
- **Phase 5**: T053 and T054 are separate mutation files; T056 alongside
  T051/T052; T058, T060 and T061 are separate web files.
- **Phase 6**: T066/T067 (the package) alongside T068–T072 (the server); T071,
  T072, T074, T075 and T076 are separate files.
- **Phases 5 and 6 side by side**: phase 5 touches `auth/actor_imagery.rs`,
  `mutations_worlds.rs` and `mutations_actors.rs`; phase 6 touches
  `heroes/spec_schema.rs`, `collections/copy.rs` and `users/export_content.rs`.
  They meet only in `mutations_actor_images.rs`, so whichever runs second
  rebases that one file.
- **Never in parallel**: two e2e runs, or `cargo test` beside one (shared
  database and global settings rows).

## Implementation strategy

**First shippable slice: Phase 3** — the standalone builder. It clears SC-001,
answers the owner's "for us to generate", and needs no server, no migration and
no review of an authorisation rule. It is also where the builder's layout,
keyboard behaviour and performance are settled before any player sees them.

**The owner's 2026-09-15 request lands in Phase 4**, the second shipping phase.
There is no earlier honest place for it: the button opens a builder, and
Phase 3 is the builder. Phase 4 is deliberately small because the control lives
in `ActorImageryPanel` rather than in a page — one component's change puts it on
the NPC editor and on a character's page at once.

**MVP: Phases 2–4.** A developer tunes heroes without writing code; a Game
Master gives any NPC a face from inside the product; a named NPC with art takes
three interactions. That is US1–US4 and SC-001 to SC-009 complete.

**Then Phase 5** (US5, SC-010) and **Phase 6** (US6/US7, SC-011), each
shippable alone and in either order.

## Notes

<!-- T002 baseline, 2026-09-15, main at d3b91d7. The catalogue as shipped:
       HERO_PARTS       12  ears, mouth, hair, headgear, emblem, prop,
                            build, muzzle, eyeStyle, hide, wings, tail
       HERO_COLORS      12  skin, eyes, hairColor, beardColor, headgearColor,
                            accent, outfit, trim, glow, ring, hideColor, wingColor
       HERO_FLAGS        2  tusks, beard
       SIZE_CATEGORIES   6  tiny .. gargantuan (tiny 0.5, gargantuan 4 cells)
       SKIN_TONES        8   MONSTER_TONES 12   PRESET_HEROES 12   BESTIARY 30
     The spec's context says six parts and ten colours; that was true on
     2026-09-10 and stopped being true on 2026-09-12 (commit 5239839, "Give
     the bestiary faces"). The e2e derives these at run time and must never
     restate them. -->

- Commits are signed, stage explicit paths, and each names the phase and task ids.
- `.e2e-shards-durations.json` is rewritten by the harness; don't commit it with feature work.
- Migrations get real timestamps when written; `<date>` above is a placeholder, not a directory name.
- ADR numbers are taken when the ADR is written. 104 and 105 are free as of 2026-09-15; specs 052–055 are being written in parallel and may take them first.
- Two questions are left for the owner in plan.md: whether the builder offers a `size` control (this plan says yes), and what should happen to stored specs if a choice is ever retired despite FR-038.
