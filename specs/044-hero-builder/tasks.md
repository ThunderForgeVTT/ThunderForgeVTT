---
description: "Task list for spec 044, the hero builder"
---

# Tasks: The Hero Builder

**Input**: Design documents from `/specs/044-hero-builder/`

**Prerequisites**: spec.md (decided 2026-09-14, clarified 2026-09-16),
research.md (R1–R7), plan.md (re-planned 2026-09-16 at `3101e9f`),
data-model.md, contracts/builder.md, quickstart.md

**Regenerated 2026-09-16** from the re-planned artifacts. None of the 87 tasks in
the 2026-09-15 list had been started, so this list replaces it rather than
amending it; task ids have moved.

**Tests**: Required. Each plan phase ends with its e2e run and the log searched
for `✘`. Contract rules B1–B9 and B5a (contracts/builder.md §5) each get a test:
B1–B3 and B5a in phase (a), B4–B5 in the phase (b) e2e, B6–B7 and B9 in
`cargo test`, B8 in both.

**No playtest.** No playtest scenario builds a look, and writing one to satisfy
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

- [X] T001 Read `specs/044-hero-builder/contracts/builder.md`, `research.md` (especially R6 and R7) and `plan.md` into the working set; every task below is measured against contract rules B1–B9 and B5a and the six findings in plan.md's "What the code says, against what the spec assumed"
- [X] T002 [P] Re-check the catalogue baseline recorded in this file's Notes section against `packages/heroes/src/spec.ts` (12 parts at `:91-104`, 12 colour fields at `:128-141`, 2 flags at `:144`, 6 sizes at `:114-123`, 8 skin tones, 12 monster tones) and update the note if it has moved; the e2e never restates these numbers
- [X] T003 [P] Confirm `pnpm-workspace.yaml`'s `apps/*` and `packages/*` globs already admit `apps/hero-builder` and `packages/hero-builder`, so no workspace file changes; note it here

---

## Phase 2: (a) The catalogue tells the builder everything (Priority: P1) — US1, US2

**Goal**: `packages/heroes` exports labels, palettes, race looks, a seeded race-aware randomiser, a minimal-spec function and a preset writer, so the builder holds no list of its own and a missing label or a broken race look fails the package rather than a screen three layers away.

**Independent test**: with no builder in existence, `pnpm -F @thunderforge/heroes check` passes; deleting one label from `HERO_LABELS`, or pointing a race at a choice that does not exist, makes it fail by naming the field.

- [X] T004 [P] [US2] Add `HERO_LABELS` in new `packages/heroes/src/labels.ts` per contracts §1: `fields` has a label for every key of `HERO_PARTS`, `HERO_COLORS` and `HERO_FLAGS` plus `name`, `title`, `size` and `race`; `choices` labels every choice of every part and every size category; `races` labels every key of `HERO_RACES` ("half-orc" → "Half-orc")
- [X] T005 [P] [US1] Add `HERO_PALETTES` in new `packages/heroes/src/palettes.ts`: `skin` → `SKIN_TONES`' values, `hideColor` → `MONSTER_TONES`' values, and a curated `#rrggbb` list for the other ten colour fields; every entry must satisfy `HEX_COLOR` (`packages/heroes/src/color.ts:4`)
- [X] T006 [P] [US1] Add `HERO_RACES`, `RaceKey` and `matchRace(text)` in new `packages/heroes/src/races.ts` per contracts §1 and research R6's table (human, elf, half-elf, dwarf, halfling, gnome, orc, half-orc, tiefling, dragonborn, goblin): each entry holds lower-case `aliases` and only the `choices`, `swatches` and `flags` that make the race recognisable, built from parts that exist today (`spec.ts:17-79`) — no new drawing. `matchRace` trims, lower-cases and compares against each key and alias; `"High Elf"` → `"elf"`, `"Moonkin"` → `null`, empty or nullish → `null` (FR-007a)
- [X] T007 [US1] Add `randomHero(seed, options?: { locked?, race? })` in new `packages/heroes/src/random.ts`, built on the existing `seeded()` chooser (`packages/heroes/src/seed.ts:56`): choices only from `HERO_PARTS`, colours only from `HERO_PALETTES`, each field the race constrains picked from the race's narrower list instead, and flags the race fixes set to its value; a locked field is omitted from the result so the caller's value survives a merge — a lock beats a race (FR-007, FR-007a); the race is folded into the seed's input so `(seed, race)` reproduces the hero; no `Math.random`
- [X] T008 [US1] Add `minimalSpec(spec)` in new `packages/heroes/src/minimal.ts`: drop a field exactly when removing it leaves `resolveHero()` unchanged, so the six derived colours (`packages/heroes/src/spec.ts:319-335`) keep following; `name` is always kept (FR-009)
- [X] T009 [US1] Add `presetSource(slug, spec)` in new `packages/heroes/src/presetSource.ts`: a `HeroPreset` entry as TypeScript source, writing `SKIN_TONES.<name>` where the skin matches a named tone (FR-014)
- [X] T010 [US1] Re-export T004–T009 from `packages/heroes/src/index.ts` beside the existing exports; add nothing to `package.json` (the package stays dependency-free)
- [X] T011 [P] [US2] Test in `packages/heroes/src/labels.test.ts`: every field (including `race`), every choice and every race key has a label, and a deliberately removed label fails by naming the field (US2 scenario 3)
- [X] T012 [P] [US1] Test in `packages/heroes/src/races.test.ts`: every constrained choice in `HERO_RACES` is a real choice of that part (or a real size category), every swatch is `#rrggbb`, no alias belongs to two races and no alias equals another race's key, and `matchRace` returns its race for every key and every alias in any casing with surrounding whitespace, and `null` for unknown text
- [X] T013 [P] [US1] Test in `packages/heroes/src/random.test.ts`: every `randomHero` result passes `validateHero`; one `(seed, race)` always gives one hero; a locked field is absent from the result; no colour is produced that is not in `HERO_PALETTES`; for every race and fifty seeds, the rolled hero satisfies every constraint in that race's look, derived from `HERO_RACES` at run time; a field locked against its race's constraint stays absent; and `resolveHero` of a race-rolled spec contains no `race` key (B5a)
- [X] T014 [P] [US1] Test in `packages/heroes/src/minimal.test.ts`: `resolveHero(minimalSpec(s))` equals `resolveHero(s)` for all twelve presets and for a hundred seeded random heroes (with and without a race); a set field equal to its derived default is dropped; `presetSource` output names a `SKIN_TONES` entry where one matches
- [X] T015 [US1] Prove: `pnpm -F @thunderforge/heroes check` and `pnpm verify` (whose `heroes` step is that command, `scripts/verify.mjs:144-147`); record results here
  - 2026-09-16: `heroes check` 41 pass / 0 fail (tsc clean); `pnpm verify` 14/14. T002 baseline unchanged; T003 globs admit both new members, no workspace change. `randomHero` returns `RolledHero` (a spec without its name — the name is the caller's) and rolls a person: creature parts and size stay default unless a race asks.

**Checkpoint**: everything a builder needs to draw its own controls and roll by race exists, and nothing renders it yet.

---

## Phase 3: (a) A look we can see while we change it (Priority: P1) — US1, US2

**Goal**: `packages/hero-builder` renders the whole catalogue from data, with a race picker beside the dice, and `apps/hero-builder` hosts it with no server, no auth and no GraphQL.

**Independent test**: with only the standalone app running, load a preset, change one choice of every part, roll as an elf and see pointed ears, export, import the export back, and confirm it draws exactly what was on screen.

- [X] T016 [US1] Create `packages/hero-builder/package.json` (`@thunderforge/hero-builder`, private, `type: module`, `"." → ./src/index.ts` as `packages/heroes` does), `tsconfig.json` and `src/index.ts` exporting exactly contracts §2 (`HeroBuilder`, `HeroBuilderProps`, `renderHero`, `HeroPreview`); React 19 and `react-dom` as **peer** dependencies only, `@thunderforge/heroes` as a workspace dependency, and nothing imported from `apps/web`
- [X] T017 [US1] `packages/hero-builder/src/preview/HeroPreview.tsx` and `renderHero(spec, idPrefix)`: portrait and token for a spec, each SVG given an id prefix derived from the caller's `idPrefix` (which must satisfy `^[A-Za-z][\w-]{0,63}$`, `render.ts:62`) so no two elements in the host document share an id (FR-011, B2)
- [X] T018 [US1] `packages/hero-builder/src/controls/`: one control kind per field kind, all generated from `HERO_PARTS`, `HERO_COLORS`, `HERO_FLAGS`, `SIZE_CATEGORIES` and `HERO_LABELS` — a part is one arrow-key group (FR-017), a colour is `HERO_PALETTES` swatches plus a `#rrggbb` field whose hex is readable as text (FR-008, FR-018), a flag is a switch, plus name, title and size; a choice with no label shows its key (FR-002). No part, choice or colour key appears in this directory's source (B1)
- [X] T019 [US1] Set-versus-following in `packages/hero-builder/src/controls/`: a field the user has not set shows its resolved value as "following", and a set field can be returned to following; decided by comparing `resolveHero(spec)` with `resolveHero(minimalSpec(spec))`, never by a list of derived fields (FR-005)
- [X] T020 [US1] `packages/hero-builder/src/roll/`: the race picker and the dice (FR-007a) — a picker generated from `HERO_RACES` and `HERO_LABELS.races` with "any" first, starting on `initialRace` when it is a known key and on "any" otherwise; a dice button calling `randomHero(seed, { locked, race })` with a fresh seed; the seed shown and re-enterable; per-field locks. The race is held as roll state only and is never written into the spec, passed to `onChange`, or included in export or copy-as-preset (B5a)
- [X] T021 [US1] `packages/hero-builder/src/HeroBuilder.tsx`: state is a `HeroSpec`, every spec passes `validateHero` before anything is drawn, an invalid one is reported by field through `onInvalid` and changes nothing on screen (FR-004, FR-010, B3); continuous input redraws at most once per animation frame (FR-021); mounts the controls, the roll bar and both previews; the props are exactly contracts §2, including `initialRace`
- [X] T022 [US1] `packages/hero-builder/src/io/`: export the portrait SVG, the token SVG and `minimalSpec` as JSON, each a file the user saves (FR-013); import a spec from pasted text or a file, refused whole with every problem named (FR-010); "copy as preset" puts `presetSource` output on the clipboard (FR-014). No SVG import exists anywhere in this directory (FR-026)
- [X] T023 [P] [US2] B1 guard in new `packages/hero-builder/src/catalogue-free.test.ts` (run by the package's `check` script): grep `packages/hero-builder/src` for a sample of part keys, choice keys and race keys read from `@thunderforge/heroes` at run time (`"headgear"`, `"wizard"`, `"elf"`, …) and fail naming the file that contains one
- [X] T024 [US1] Create `apps/hero-builder`: `package.json` (`@thunderforge/hero-builder-app`, scripts `dev`, `build`, `preview`, `test:e2e`), `vite.config.mts` on the web app's Vite with a port of its own, `index.html`, `src/main.tsx`. No server, no auth, no GraphQL, no `@/…` alias into `apps/web` (FR-012)
- [X] T025 [US1] The standalone app's chrome in `apps/hero-builder/src/App.tsx`: a preset picker over `PRESET_HEROES` (FR-006), `HeroBuilder` with `initialRace={null}` so the picker starts on "any" (FR-007a), and the export/copy actions passed through `actions`
- [X] T026 [US1] `apps/hero-builder/playwright.config.ts`: `webServer` runs this app's own `preview`, one worker, no global setup, no database — and a note saying why this suite is not in `scripts/e2e-parallel.mjs` (its two suites are both rooted at `apps/web`, `scripts/e2e-parallel.mjs:185-193`, and every lane stands up a stack this app does not use)
- [X] T027 [US1] [US2] `apps/hero-builder/e2e/builder.spec.ts` part 1: derive the expected controls from `HERO_PARTS`, `HERO_COLORS`, `HERO_FLAGS` and `SIZE_CATEGORIES` **at run time** and assert one labelled, selectable group per field, with no count written in the spec file (FR-001, FR-002, SC-002)
- [X] T028 [P] [US1] `builder.spec.ts` part 2: every control change redraws both previews with no reload (FR-003), and the change is reflected within 100 ms measured in the page and logged per run (SC-006)
- [X] T029 [P] [US1] `builder.spec.ts` part 3: every preset loads and draws exactly what `packages/heroes/src/cli.ts` writes for that preset (US1 scenario 3); export then import reproduces the on-screen hero byte for byte for every preset (SC-003)
- [X] T030 [P] [US1] `builder.spec.ts` part 4: no duplicated element id on the page after loading every preset (SC-005, B2); an invalid pasted spec lists every problem by field and changes nothing (FR-010)
- [X] T031 [P] [US1] `builder.spec.ts` part 5: the whole builder, including the race picker and dice, walked by keyboard alone, every choice and swatch with an accessible name and its hex readable as text (SC-004, FR-017, FR-018); at 375 px both previews stay visible, nothing scrolls sideways, every target is at least 44 px (FR-020)
- [X] T032 [US1] `builder.spec.ts` part 6: the dice are reproducible from the shown seed and leave a locked part unchanged (FR-007); "copy as preset" produces an entry that, pasted into `presets.ts`, passes `pnpm -F @thunderforge/heroes check` and draws identically (FR-014, SC-001)
- [X] T033 [US1] `builder.spec.ts` part 7, the race roll (FR-007a, B5a): the picker starts on "any" and lists every key of `HERO_RACES` after it, derived at run time; for every race, rolling with three seeds draws a hero whose exported spec satisfies that race's look; a part locked against the race keeps its value; the exported JSON and the copied preset contain no race
- [X] T034 [US2] The SC-002 proof, by hand and once: on a throwaway branch add a choice to one list in `HERO_PARTS` with its label, run T015's and T036's commands unchanged, confirm the e2e finds, selects and draws it with **no** edit to `packages/hero-builder` or to `builder.spec.ts`; record the result here and discard the branch
- [X] T035 [P] [US1] `packages/hero-builder/README.md` and `apps/hero-builder/README.md`: what each is, how to run the app, that the library takes React from its host, and that race looks live in `packages/heroes` — in the manner of `apps/engine-sandbox/README.md`
- [X] T036 [US1] Prove: `pnpm -F @thunderforge/hero-builder-app test:e2e`, `pnpm -F @thunderforge/hero-builder check` (T023) and `exec tsc --noEmit`, `pnpm -F @thunderforge/heroes check` and `pnpm verify`; record results and the measured SC-006 figure here

  > 2026-09-16: `hero-builder-app test:e2e` **7/7 pass** (≈40 s, one worker). **SC-006: 22 control changes, median 24.4–25.5 ms, worst 26.2–31.6 ms** from click to both pictures redrawn, measured in the page over four runs. `hero-builder check` 2/2 with tsc clean; the app's `tsc --noEmit` clean; `heroes check` 41/0; `pnpm verify` **15/15** — the builder's check is now its own verify step ("hero builder package", beside "heroes package"), so B1's guard runs without anyone remembering it.
  >
  > **T034 (SC-002), done once:** added `star` to `EMBLEMS` with its label and a drawing, ran `heroes check` (41/0), `hero-builder check` (2/0) and the full e2e (7/7) with no edit to `packages/hero-builder` or `builder.spec.ts`, then restored the three files (`git checkout --`, in place of a throwaway branch). The run exposed that part 1 only *selected* one choice per field, so part 1 now selects every choice and asserts each one redraws the portrait; re-run with `star` present it found, selected and drew it.
  >
  > **T032 by hand, once:** a dice roll's `presetSource` pasted into `presets.ts` passes `heroes check` (41/0) and was reverted. It needs a title: `heroes.test.ts` holds every preset to one, so a preset copied from a hero with no title fails that test until one is typed. The e2e proves the copied entry draws byte-identically; the paste itself stays a manual step.
  >
  > **Differences from the contract, now written into contracts/builder.md:** the index also exports the file helpers (`heroFiles`, `parseHeroText`, `saveHeroFile`, `fileStem`, `HeroProblem`) — the library still performs no action unasked; the standalone app wires them into `actions`. Exported SVGs use the renderer's default ids, so an exported preset is byte-identical to `cli.ts`'s output; the on-screen pictures use `idPrefix`. A roll keeps the name, the title and the locked fields and replaces everything else, so `(seed, race, locks)` always gives the same hero. Styles ship inside the component, scoped under `.tfhb`, so no host needs a stylesheet or a Tailwind source for it.
  >
  > **Chromium drops downloads:** clicked back to back, about one download in eleven never fires (reproduced with one button, 24 clicks: #10 and #21 lost). The suite's `download` helper tries a click twice. Separately, `saveHeroFile` now removes its link and revokes the blob URL 30 s later instead of at once.

**Checkpoint**: SC-001 — a new preset reaches `presets.ts` in under five minutes without writing code. The owner's first stated use is done.

---

## Phase 4: (b) A Game Master gives an NPC a face (Priority: P1) — US3, US4

**Goal**: "Build look" inside the imagery panel (so it appears on the NPC editor and the actor page at once) and on every NPC row of the compendium, all opening one full-screen dialog with the race read from the sheet, all saving through one helper; and Quick NPC beside "New NPC". **This is the phase that answers the owner's request of 2026-09-15.**

**Independent test**: as a Game Master, open a dnd5e NPC whose race is "High Elf", press "Build look", confirm the dialog opens full-screen with no change of URL and the picker on elf, save, and confirm the portrait and token are stored and served as WebP; do the same from the NPC's compendium row without leaving the list; then make three NPCs with Quick NPC and confirm three different faces.

### The race on the sheet (FR-007a, research R7)

- [ ] T037 [P] [US3] New `crates/pack_system_spec/src/appearance.rs`: `SystemAppearance { race: Option<SystemAppearanceRace> }` and `SystemAppearanceRace { source: SystemFieldRef }` (reusing `combat.rs:75`'s `SystemFieldRef`), `JsonSchema` + serde derives with `deny_unknown_fields` as the neighbouring blocks do; a `validate_appearance` that checks `race.source` with `require_field` — make `require_field` (`combat.rs:151`) `pub(crate)` rather than copying it
- [ ] T038 [US3] Add `pub appearance: Option<SystemAppearance>` to `SystemManifest` in `crates/pack_system_spec/src/lib.rs:53-110`, declare `mod appearance`, and call its validation wherever `combat` is validated so an undeclared slot or field is refused at pack load; the published schema (`get_system_manifest_schema`, `lib.rs:216`) picks it up by derivation
- [ ] T039 [P] [US3] `cargo test -p pack_system_spec` in new `crates/pack_system_spec/src/appearance_tests.rs`: a manifest with no `appearance` validates; `traitData.race` on a system declaring it validates; an undeclared slot and an undeclared field are each refused with the path `appearance.race.source` in the message; an unknown key inside `appearance` is refused; the bundled dnd5e and Genie manifests both validate
- [ ] T040 [US3] Declare `"appearance": { "race": { "source": { "slot": "traitData", "field": "race" } } }` in `packs/systems/dnd5e/system.json` (race field at `:855-858`); Genie's manifest is left without the block
- [ ] T041 [P] [US3] New `apps/web/src/utils/raceOnSheet.ts`, modelled on `apps/web/src/utils/sizeCategory.ts:30-50`: `raceSourceOf(manifest)` returns the declared `appearance.race.source` or null, and `raceOnSheet(manifest, systemData)` returns the string at that slot and field or null — naming no system; plus `apps/web/src/utils/raceOnSheet.test.ts` covering declared, undeclared, missing value and non-string value

### One dialog, one save (FR-019, FR-024, FR-025)

- [ ] T042 [US3] Add `@thunderforge/hero-builder` and `@thunderforge/heroes` as workspace dependencies of `apps/web`, and whatever `apps/web/vite.config.mts` needs to resolve them from source; the library must take the app's React, so check that the existing force-resolution block (`apps/web/vite.config.mts:41-47`, written for `@thunderforge/genie`) is not defeated and that no second React reaches the page
- [ ] T043 [P] [US3] A single helper in new `apps/web/src/pages/world/actor/heroFiles.ts` turning an SVG string and a role into a `File` of type `image/svg+xml`; no new mutation, no new endpoint, no browser rasterisation (FR-024, B4)
- [ ] T044 [US3] New `apps/web/src/pages/world/actor/saveBuiltHero.ts`: `saveBuiltHero(actorId, spec)` renders both SVGs with `renderHero`, uploads portrait then token through the existing `uploadActorImage` (`apps/web/src/api/actors.ts`) using T043's files, and returns a per-role result (`saved` with the returned row, or `failed` with the server's message); no role is `saved` unless the mutation returned its row (FR-025, B4); a paused-world refusal (`refuse_content_if_paused`, `src/server/src/graphql/mutations_actor_images.rs:122-124`) comes back as `failed` for both roles with that message and a `paused` flag; a `retry(role)` re-uploads that role alone
- [ ] T045 [US3] New `apps/web/src/pages/world/actor/HeroBuilderDialog.tsx` taking `{ worldId, actorId, actorLabel, existingRoles, onSaved }`: a **full-screen** dialog over the current page that never changes the URL (FR-019); `import()`s `@thunderforge/hero-builder` at open time and nowhere else (FR-022); moves focus in, traps it, and returns it to the opener; opens on a spec named `actorLabel` (FR-023); reads the race itself via `useActorSystemData(actorId)` and the world's manifest → `raceOnSheet` → `matchRace` → `initialRace` (FR-007a)
- [ ] T046 [US3] The save step in `HeroBuilderDialog.tsx`: where `existingRoles` is non-empty, say before the upload starts that both portrait and token will be replaced (FR-025); call `saveBuiltHero`; show each role's result with a retry for a failed role; on a paused world show the server's message, keep the dialog open with the hero intact, offer export, and report nothing saved (spec Edge Cases); call `onSaved` with the returned rows
- [ ] T047 [US3] Add the "Build look" control (`data-testid="actor-imagery-build"`) to `apps/web/src/pages/world/actor/ActorImageryPanel.tsx`, rendered exactly when `canEdit` — the condition that already governs the file inputs (`:182`) — opening `HeroBuilderDialog` with the actor's label and the panel's existing roles, and refreshing both roles from `onSaved`. This reaches `NpcEditorPage.tsx:224` and `ActorDetailPage.tsx:489-494` without touching either page
- [ ] T048 [US3] Remove the reserved builder comment in `apps/web/src/pages/world/actor/ActorDetailPage.tsx` (`:~496-501`), since the panel now carries the control; change nothing else on the page
- [ ] T049 [US3] Add "Build look" (`data-testid="npc-catalog-build-${id}"`) to each row of `apps/web/src/pages/world/compendium/NpcCompendiumTab.tsx`, beside the row's portrait upload (`:283-352`) and on its condition `npc.myPermissionLevel !== "VIEWER"` (`:296`), opening `HeroBuilderDialog` for that NPC; on save update `imagesByActor[id]` from the two returned rows the way `handlePortrait` does (`:177-200`), leaving the GM on the list; the row's existing upload is unchanged (FR-028a)

### Quick NPC (FR-027, FR-028)

- [ ] T050 [US4] New `apps/web/src/pages/world/compendium/QuickNpcDialog.tsx`: a name field, the `HeroPreview` of a `randomHero` rolled with race "any" (a new NPC has no sheet yet), a reroll, **a reserved space below the name for a later system NPC template** (FR-027, Decisions 2), "Open in builder" handing the current hero to `HeroBuilderDialog`, and Create
- [ ] T051 [US4] Add the "Quick NPC" control (`data-testid="quick-npc"`) beside the "New NPC" button (`data-testid="new-npc-link"`) in `apps/web/src/pages/world/compendium/NpcCompendiumTab.tsx:397-407`, on the same `isGm` condition, so a Player is offered none
- [ ] T052 [US4] Quick NPC's create path in `QuickNpcDialog.tsx`: `createActor({ worldId, label, isNpc: true })` then `saveBuiltHero`; the Game Master stays in the compendium with the new NPC selected (US4 scenario 2); where an upload fails the NPC **remains**, is reported as lacking art with its row's "Build look" named as the way to add it, and the dialog reports no success (FR-028)
- [ ] T053 [P] [US4] Show an NPC with no token or portrait as lacking art in `NpcCompendiumTab.tsx`, using the `imagesByActor` it already holds, with an accessible label and the row's "Build look" beside it

### Proof

- [ ] T054 [P] [US3] e2e `apps/web/e2e/hero-builder-npc.spec.ts` part 1: a Game Master opens an NPC's edit page, presses "Build look", the dialog is full-screen and `page.url()` is unchanged (FR-019); the builder opened with the NPC's name (FR-023); they change a choice and save; both roles appear in the panel; the served bytes are WebP (`RIFF`/`WEBP`), asserted the way the P8 spec does (`apps/web/e2e/world-compendium.spec.ts:268`) (SC-007)
- [ ] T055 [P] [US3] `hero-builder-npc.spec.ts` part 2, the row (FR-028a): on the compendium list a row's "Build look" opens the same dialog, saves both roles, and the GM is still on the list with that row's portrait updated; a Player sees no row build control
- [ ] T056 [P] [US3] `hero-builder-npc.spec.ts` part 3, the race (FR-007a): a dnd5e NPC whose sheet race is "High Elf" opens with the picker on elf and a roll has pointed ears; an NPC with race "Moonkin" and a Genie NPC both open on "any"
- [ ] T057 [P] [US3] `hero-builder-npc.spec.ts` part 4: an NPC that already has art warns before saving that both roles will be replaced; one role forced to fail reports that role failed and the other saved, and the failed role retries alone (FR-025); closing without saving leaves the images unchanged; with the world paused, saving reports the refusal, stores nothing, and the dialog keeps the hero
- [ ] T058 [P] [US3] `hero-builder-npc.spec.ts` part 5: a Viewer on the same page is offered no build or save control **and** a direct `uploadActorImage` call is refused (US3 scenario 5, B5)
- [ ] T059 [P] [US4] `hero-builder-npc.spec.ts` part 6: three NPCs from an empty compendium through Quick NPC, each named with a portrait and a token and all three faces different (SC-008); reroll changes the face; "Open in builder" carries the current hero; a Player is offered no Quick NPC; a failed upload leaves the NPC listed as lacking art with its row's "Build look" (FR-028)
- [ ] T060 [US3] `hero-builder-npc.spec.ts` part 7, the two rules a builder must not weaken: a built NPC placed twice has both tokens wearing the face, because a copy reads its actor's art (`src/server/src/graphql/token_art.rs:41-58`, ADR-102); and a hidden NPC's built portrait is refused to a player while its token art is still served on a scene they can see (`src/server/src/assets_serve/actor.rs:155-165`)
- [ ] T061 [P] [US3] Prove SC-012 and FR-022 from the built chunk graph: `pnpm -F @thunderforge/web build`, then show the builder's code is in no chunk loaded by a page that does not open it (including the compendium list, which only renders the row control); record the figures here
- [ ] T062 [US3] Prove: `node scripts/e2e-parallel.mjs --shards=1 --only=hero-builder-npc,world-compendium` with the log searched for `✘`; `cargo test -p pack_system_spec`; `pnpm -F @thunderforge/web exec tsc --noEmit`; `pnpm verify`; record results and the SC-007/SC-008 timings here

**Checkpoint**: the owner's 2026-09-15 request is answered — "Build look" sits beside the portrait and token on an NPC's edit page and on its compendium row, rolls by the sheet's race, and a named NPC with a face takes three interactions.

---

## Phase 5: (c) A player's own character (Priority: P2) — US5

**Goal**: the player who holds a character may change its look and nothing else; a Game Master may turn that off for a world or lock one character; a Game Master's own authority is untouched.

**Independent test**: as an invited player, create your own character with a built look; confirm a second player cannot change it, that the world setting off refuses it, that a lock refuses that one character only, and that the Game Master can still replace the art.

- [ ] T063 [US5] Write `docs/adrs/<date>-105-a_characters_look_belongs_to_whoever_holds_it.md` (PROPOSED) from plan.md's Constitution Check and contracts §5 B6: why a claim confers a right that ADR-050's ladder does not, why the right is imagery-only rather than an Editor grant row, and why the two withdrawals are a world setting and a per-character lock rather than an approval queue (FR-030c); add the row to `docs/adrs/README.md`. Take the next free number at the time of writing — 105 is free as of 2026-09-16
- [ ] T064 [US5] Migration `src/server/migrations/<date>-044-player-actor-art/` (up/down) per data-model.md: `worlds.allow_player_actor_art BOOLEAN NOT NULL DEFAULT true` backfilled `true` for every existing world, and `world_actors.art_locked BOOLEAN NOT NULL DEFAULT false`; regenerate `src/server/src/schema.rs`
- [ ] T065 [US5] Set the hand-kept creation defaults to match: `allow_player_actor_art: true` in `src/server/src/graphql/mutations_worlds.rs` world creation and in the adapters, so a new world agrees with every old one (data-model.md notes why this default is the opposite of its neighbours)
- [ ] T066 [US5] New `src/server/src/auth/actor_imagery.rs`: `may_change_actor_imagery(conn, user, actor_id)` implementing B6 — Editor or above by the existing ladder, **or** a live claim in `world_actor_claims` on an actor whose world has `allow_player_actor_art = true` and whose `art_locked` is false; returns the three distinct refusals named in contracts §5. Register the module in `src/server/src/auth/mod.rs`
- [ ] T067 [US5] Call it in place of `require_actor_permission(Editor)` in both `upload_actor_image_impl` and `remove_actor_image_impl` in `src/server/src/graphql/mutations_actor_images.rs:113-121` and the remove path; the pause gate (`:122-124`) stays exactly as it is, and no other actor mutation changes
- [ ] T068 [P] [US5] `updateWorldAllowPlayerActorArt` in `src/server/src/graphql/mutations_worlds.rs`, following `update_world_auto_apply_npc_damage_impl` (`src/server/src/graphql/mutations_attacks.rs`): `is_dm_of_world`, `refuse_if_paused` inside the blocking closure, one `diesel::update` returning `GraphQLWorld` (FR-030a)
- [ ] T069 [P] [US5] `setActorArtLocked(actorId, locked)` in `src/server/src/graphql/mutations_actors.rs`, Game Master only, beside the existing `setActorUnique`/visibility setters; locking changes no image (FR-030b)
- [ ] T070 [US5] Expose `allowPlayerActorArt` on `GraphQLWorld`, and `artLocked` plus `myMayChangeImagery: Boolean!` (B6 evaluated for the caller via T066) on `GraphQLWorldActor` (`src/server/src/graphql/types_scene.rs`); add both setters to `play_pause_surface_tables.rs` where the other setters are listed
- [ ] T071 [P] [US5] `cargo test` in new `src/server/src/auth/actor_imagery_tests.rs`, one per clause of B6: a claimant may write imagery and a non-claimant may not; a player who created their own character may, and stops when the claim ends; the grant reaches no other actor mutation; the world setting off refuses every actor in that world; a lock refuses that actor whatever the setting; a Game Master is refused by neither and may replace a player's art; the three refusals carry three different messages; `myMayChangeImagery` agrees with the gate in every case
- [ ] T072 [US5] Regenerate `src/app/schema.graphql` (`node scripts/check-graphql-contract.mjs --schema --fix`); `pnpm verify` green including `graphql-schema` and `graphql-ops`
- [ ] T073 [P] [US5] Add `updateWorldAllowPlayerActorArt` and `setActorArtLocked` to `apps/web/src/api/world.ts` and `apps/web/src/api/actors.ts`, and carry `allowPlayerActorArt`, `artLocked` and `myMayChangeImagery` on the records the pages already read
- [ ] T074 [US5] Mount `ActorImageryPanel` in **view mode** on `apps/web/src/pages/world/actor/ActorDetailPage.tsx`, with `canEdit={actor.myMayChangeImagery}`, because edit mode redirects a Viewer to `/view` (`:140-142`) and a claim holder resolves to Viewer (plan finding 5); in edit mode switch the existing mount's `canEdit` (`:144`, `:489-494`) to the same field so the two modes agree (FR-033). The panel's "Build look" follows with no further change
- [ ] T075 [P] [US5] The Game Master's lock control on `ActorDetailPage.tsx`, beside the existing unique and visible-to-players toggles, with an accessible label saying it locks the look and not the character (FR-030b)
- [ ] T076 [P] [US5] The world setting "Players may change their character's art" wherever the world's other Game-Master settings live under `apps/web/src/pages/world/`, defaulting on and explaining what it withdraws (FR-030a)
- [ ] T077 [US5] Let "Create your own character" build a look before creating in `apps/web/src/pages/world/ActorSelectionPage.tsx:101-124`: create through `createAndClaimActor` first, then `saveBuiltHero`, and on a failed upload leave the character without art and say where to add it (FR-034, FR-028)
- [ ] T078 [P] [US5] e2e `apps/web/e2e/hero-builder-player.spec.ts` covering the independent test from the player's view page (no edit route), every refusal attempted **both** by looking for the control and by calling the mutation directly (SC-010): a second player refused, the world setting off refused, one character locked and another of the same player's unaffected, the claim released and then refused, each refusal showing its own message, and the Game Master replacing the art regardless
- [ ] T079 [US5] Prove: `node scripts/e2e-parallel.mjs --shards=1 --only=hero-builder-player,hero-builder-npc,actor-claims,world-actor-permissions` with the log searched for `✘`; `cargo test -p thunderforge-server --lib`; `tsc --noEmit`; `pnpm verify`; record results here

**Checkpoint**: a player gives their own character a face, and a Game Master has two ways to stop them and one way to overrule them.

---

## Phase 6: (d) A saved look re-opens as a hero (Priority: P2/P3) — US6, US7

**Goal**: the spec that drew an image is stored with that image, travels with a collection copy and a personal export, and re-opens in the builder with every choice intact.

**Independent test**: build and save a look for an NPC, reload, re-open the builder and confirm every control shows the saved choice; then copy the NPC through a collection into a second world and re-open it there.

- [ ] T080 [US6] Write `docs/adrs/<date>-106-a_stored_image_remembers_the_spec_that_drew_it.md` (PROPOSED) from research R4 and data-model.md, recorded as an extension of ADR-057: why the spec hangs on the image row and not on the actor, the rejected alternatives, the duplication accepted, the server-side schema check, the append-only consequence for the catalogue (FR-038), and that the roll's race is never stored (B5a); add the row to `docs/adrs/README.md`. Take the next free number at the time of writing
- [ ] T081 [P] [US6] Add `HERO_SPEC_SCHEMA` (JSON Schema, draft 2020-12) in new `packages/heroes/src/schema.ts`, derived from `HERO_PARTS`, `HERO_COLORS`, `HERO_FLAGS`, `SIZE_CATEGORIES` and the text bounds `validateHero` applies, with `additionalProperties: false` so a `race` key is refused; export it from `index.ts`
- [ ] T082 [P] [US6] Test in `packages/heroes/src/schema.test.ts`: the schema and the lists agree (a choice added to any list and absent from the schema fails); every preset validates against it; each case `validateHero` refuses is refused by the schema too
- [ ] T083 [US6] Migration `src/server/migrations/<date>-044-image-hero-spec/` (up/down): `world_actor_images.hero_spec JSONB NULL`; regenerate `src/server/src/schema.rs` and the image model in `src/server/src/models.rs`
- [ ] T084 [US6] New `src/server/src/heroes/spec_schema.rs` implementing B7: refuse anything that is not a JSON object, anything over 4 KB serialised, and anything failing `HERO_SPEC_SCHEMA` (vendored into the crate, or read from the package at build time — whichever a test can keep honest); a refusal names what was wrong and stores **neither** the spec nor the image
- [ ] T085 [US6] Add the optional `heroSpec: JSON` argument to `uploadActorImage` and write it in the same upsert that stores the image in `src/server/src/graphql/mutations_actor_images.rs` (upsert at `:162-174`); an upload with no argument writes `NULL`, so replacing a built image with a file clears that role's spec (FR-035, B8)
- [ ] T086 [P] [US6] Expose `heroSpec` on `GraphQLActorImage` (`src/server/src/graphql/types_actors.rs`) and return it from `actor_images_impl`
- [ ] T087 [P] [US6] `cargo test` in new `src/server/src/heroes/spec_schema_tests.rs` for B7 and B8: an array refused; an over-cap spec refused; an unknown field refused; a valid spec stored; a refusal leaving no image row written; an upload without `heroSpec` clearing a stored one
- [ ] T088 [US7] Carry `hero_spec` through the collection copy in `src/server/src/collections/copy.rs:595-616` — the select and the insert both name columns, so it does **not** travel by itself, whatever research R4 says (FR-039)
- [ ] T089 [P] [US6] Carry `hero_spec` in the personal export: `ExportedImage` and its query in `src/server/src/users/export_content.rs:304-314` (FR-040)
- [ ] T090 [P] [US7] `cargo test` for B9 beside the existing copy tests (`src/server/src/collections/copy_asset_and_link_tests.rs`): a copied actor's images arrive with their specs, saving in the new world changes the copy only, and the export carries the spec
- [ ] T091 [P] [US6] Make the catalogue append-only now that specs are stored (FR-038): a test in `packages/heroes` over a recorded list of every choice ever shipped, failing when one is renamed or removed while allowing a new one and allowing a choice to change how it draws
- [ ] T092 [US6] Send `heroSpec` (the `minimalSpec`) with each role's upload from `apps/web/src/pages/world/actor/saveBuiltHero.ts` and `apps/web/src/api/actors.ts`, so the panel, the compendium row, Quick NPC and create-your-own all store it at once; send none from the panel's and the row's plain file inputs so an uploaded file clears that role's spec (FR-035, B8)
- [ ] T093 [US6] Open the builder from the stored spec in `apps/web/src/pages/world/actor/HeroBuilderDialog.tsx`, preferring the portrait's; where the two roles' specs differ, or only one role has one, say so; where neither has one, start from the actor's name as in phase (b); the race picker still starts from the sheet (FR-036)
- [ ] T094 [P] [US6] A stored spec that no longer validates opens with every problem named by field and the stored images untouched — nothing silently redrawn from defaults (US6 scenario 4); every client runs `validateHero` before drawing a stored spec (FR-037), in `HeroBuilderDialog.tsx`
- [ ] T095 [US6] Regenerate `src/app/schema.graphql`; `pnpm verify` green including `graphql-ops`
- [ ] T096 [US6] [US7] e2e `apps/web/e2e/hero-builder-saved.spec.ts` covering the independent test plus: a look saved from the compendium row re-opens from the edit page with the same choices; a token replaced by an uploaded file re-opens from the portrait's spec and says the token is no longer built; an actor with no stored spec opens from its name; a hand-invalidated spec shows its problems and leaves the images alone (SC-011)
- [ ] T097 [US6] Prove: `node scripts/e2e-parallel.mjs --shards=1 --only=hero-builder-saved,hero-builder-npc,collections-copy` with the log searched for `✘`; `cargo test -p thunderforge-server --lib`; `pnpm -F @thunderforge/heroes check`; `tsc --noEmit`; `pnpm verify`; record results here

**Checkpoint**: every story in the spec is real, and a look travels with the actor that wears it.

---

## Phase 7: Polish & cross-cutting

- [ ] T098 [P] Accept ADR-105 and ADR-106 once their phases are proven, dating the acceptance and naming the tasks that proved them, as ADR-101 and ADR-102 do
- [ ] T099 [P] Document the standalone builder where a developer will find it: `apps/hero-builder/README.md` linked from wherever `apps/engine-sandbox` is linked, and a line in the repository's development docs saying it needs no stack; and a line in the pack-authoring docs for `appearance.race.source`, beside `combat.sizes.source`
- [ ] T100 [P] Re-read the spec's Assumptions against what shipped and correct anything that moved, in the manner of plan.md's findings section
- [ ] T101 Full run: `node scripts/e2e-parallel.mjs --shards=2` with nothing else running; search the log for `✘`; record the time and result beside the run recorded in spec 046's T109
- [ ] T102 `pnpm verify`, `pnpm -F @thunderforge/web exec tsc --noEmit`, `make lint` and `make lint-wasm` before the final commit of the feature

---

## Dependencies

```text
Phase 1 (setup)
  └─ Phase 2 (a: the catalogue + race looks) ── blocks everything below
       └─ Phase 3 (a: the builder + dice)    ── ships first; SC-001
            └─ Phase 4 (b: a GM's NPC)       ── the owner's 2026-09-15 request
                 ├─ Phase 5 (c: a player)    ── needs 4's panel control and saveBuiltHero;
                 │                              adds authorisation and view-mode mounting
                 └─ Phase 6 (d: saved specs) ── needs 4's saveBuiltHero (something to save);
                                                independent of 5
Phase 7 (polish) ── after every phase that ships
```

Phases 5 and 6 are independent of each other and may ship in either order. Both
change the data model, and each carries its own ADR.

Within Phase 4: the manifest block (T037–T040) and `raceOnSheet` (T041) are
independent of the dialog work and may land first; `saveBuiltHero` (T044)
precedes the dialog (T045–T046), which precedes every host (T047, T049, T050).

Within a phase: package additions → library → host → migration → server → GraphQL
→ tests → web → e2e → proof.

## Parallel opportunities

- **Phase 2**: T004, T005 and T006 are separate new files; T011–T014 are
  separate test files and may be written alongside T007–T009.
- **Phase 3**: T020 (the roll bar) alongside T018/T019 (the controls); T023 any
  time after T016; T028–T033 are separate cases in one spec file and may be
  written alongside each other once T027 has established how the catalogue is
  derived; T035 (the READMEs) any time after T024.
- **Phase 4**: T037/T039 (Rust) and T041 (web util) alongside T042–T044; T043
  and T053 alongside the dialog; T054–T059 split across separate
  `test.describe` blocks; T061 needs only a build.
- **Phase 5**: T068 and T069 are separate mutation files; T071 alongside
  T066/T067; T073, T075 and T076 are separate web files.
- **Phase 6**: T081/T082 (the package) alongside T083–T087 (the server); T086,
  T087, T089, T090 and T091 are separate files.
- **Phases 5 and 6 side by side**: phase 5 touches `auth/actor_imagery.rs`,
  `mutations_worlds.rs`, `mutations_actors.rs` and `ActorDetailPage.tsx`; phase
  6 touches `heroes/spec_schema.rs`, `collections/copy.rs`,
  `users/export_content.rs`, `saveBuiltHero.ts` and `HeroBuilderDialog.tsx`.
  They meet only in `mutations_actor_images.rs`, so whichever runs second
  rebases that one file.
- **Never in parallel**: two e2e runs. `cargo test` uses `thunderforge_test`
  and may run beside an e2e.

### Parallel example: Phase 2

```text
T004 labels.ts      T005 palettes.ts      T006 races.ts
      └──────────────────┬───────────────────┘
                   T007 random.ts  (needs palettes + races)
T011 labels.test.ts   T012 races.test.ts   T013 random.test.ts   T014 minimal.test.ts
```

### Parallel example: Phase 4

```text
T037+T039 pack_system_spec appearance     T041 raceOnSheet.ts     T043 heroFiles.ts
T040 dnd5e system.json (after T038)       T044 saveBuiltHero.ts (after T043)
                          T045/T046 HeroBuilderDialog (after T041, T042, T044)
             T047 panel      T049 compendium row      T050–T052 Quick NPC
```

## Implementation strategy

**First shippable slice: Phases 2–3** — the standalone builder with race-aware
dice. It clears SC-001, answers the owner's "for us to generate", and needs no
server, no migration and no review of an authorisation rule. It is also where
the builder's layout, keyboard behaviour and performance are settled before any
player sees them.

**The owner's 2026-09-15 request lands in Phase 4**, the second shipping phase.
There is no earlier honest place for it: the button opens a builder, and
Phase 3 is the builder. Phase 4 stays small because the control lives in
`ActorImageryPanel` and the save lives in `saveBuiltHero` — one component puts it
on both actor pages, and one helper serves the panel, the row and Quick NPC.

**MVP: Phases 2–4.** A developer tunes looks without writing code; a Game
Master gives any NPC a face from its edit page or its compendium row, rolled to
its race; a named NPC with art takes three interactions. That is US1–US4 and
SC-001 to SC-009 and SC-012 complete.

**Then Phase 5** (US5, SC-010) and **Phase 6** (US6/US7, SC-011), each
shippable alone and in either order.

## Notes

<!-- Catalogue baseline, re-checked for T002. As of 2026-09-15, main at d3b91d7:
       HERO_PARTS       12  ears, mouth, hair, headgear, emblem, prop,
                            build, muzzle, eyeStyle, hide, wings, tail
       HERO_COLORS      12  skin, eyes, hairColor, beardColor, headgearColor,
                            accent, outfit, trim, glow, ring, hideColor, wingColor
       HERO_FLAGS        2  tusks, beard
       SIZE_CATEGORIES   6  tiny .. gargantuan (tiny 0.5, gargantuan 4 cells)
       SKIN_TONES        8   MONSTER_TONES 12   PRESET_HEROES 12   BESTIARY 30
     The e2e derives these at run time and must never restate them. -->

- Commits are signed, stage explicit paths (never `git add -A` with agents running), and each names the phase and task ids.
- Count this ledger's checkboxes case-insensitively: `[x]` and `[X]` are both done.
- `.e2e-shards-durations.json` is rewritten by the harness; don't commit it with feature work.
- Migrations get real timestamps when written; `<date>` above is a placeholder, not a directory name.
- ADR numbers are taken when the ADR is written. 105 and 106 are free as of 2026-09-16 (104 was taken by large uploads).
- The race is a roll setting, never data: nothing in phases (b)–(d) stores it, sends it, or writes it to the sheet (B5a).
- Two questions are left for the owner in plan.md: whether the builder offers a `size` control (this plan says yes; the halfling, gnome, goblin and dwarf size constraints in `HERO_RACES` depend on it), and what should happen to stored specs if a choice is ever retired despite FR-038.
- The label is "Build look" everywhere; "Build a hero" is retired.
