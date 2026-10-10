# Tasks: Bringing a Character In

**Input**: [spec.md](./spec.md), [plan.md](./plan.md), [research.md](./research.md),
[data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: the parser and the mapping are built test-first (TDD).

- In each phase, the crate tests come before the code they prove.
- The tests run against **generated, deterministic fixtures**: invented
  characters in D&D Beyond's layout (R17). No personal sheet is committed.
- The owner's real exports stay outside the repository. The example
  `measure_corpus` reads them for SC-002 and prints counts, never values.

**Proof**: each story ends with `pnpm e2e:sheet-import`, then every slice
that `pnpm e2e:which --diff` names.

- The migrations and `schema.rs` are cross-cutting, so it prints FULL SUITE.
  Run the named slices instead.
- The owner has ruled out the full suite as a gate. Never run
  `node ./scripts/e2e-parallel.mjs` on its own.
- Against the external stack, set `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1`
  and pass `--workers=1`.

**Dependency on `hotfix-player-hero-edit`**: that branch (`ca568bff`,
`cc28cd0e`) is not on `main` yet. It gives a player the way from the Players
screen to their own hero's sheet, and an editable 5e sheet. Phases 1 and 2
do not need it. **Phase 3 starts after it merges.** T041 and T042 build on
its test ids and its spec, `players-hero-edit.spec.ts`.

**File length**: `scripts/check-file-length.sh` fails a Rust file over 1000
lines. New logic goes in the new files named below. The large web files,
`ActorDetailPage.tsx` (831 lines) and the 5e `ActorSheet.tsx` (1853), take
mount points only.

**Words**: what a player brings is *content* staged under their name. A
user's shareable set is a *collection*. Only the 5e system is a *pack*.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an open
  task).
- **[Story]**: the user story a task serves:
  - **US1**: a player brings their own character (P1, the MVP);
  - **US2**: nothing is guessed in silence (P1);
  - **US3**: the world learns from a character (P1);
  - **US4**: one importer, many systems (P2);
  - **US5**: a character comes back changed (P3).

---

## Phase 1: Setup

- [X] T001 Create `crates/thunderforge-sheet-import` (lib, edition 2024, deps `thunderforge-pdf`, serde, serde_json, sha2) with empty `character.rs`, `reader.rs`, `mapping.rs`, `plan.rs` and `hash.rs`. Add it to the root workspace `Cargo.toml`.
- [X] T002 Create `packs/systems/dnd5e/sheet/` (`thunderforge-system-dnd5e-sheet`, cdylib+rlib, an optional `wasm` feature, deps `thunderforge-pdf` and `thunderforge-sheet-import`). Add it to the workspace.
- [X] T003 In `scripts/shared.mjs`, beside `buildPdf()` (:341), add `buildSheetReaders()`. It runs `wasm-pack` for each pack `sheet/` crate that has a `wasm` feature, into `dist/sheet-<system>`, as `@thunderforge/sheet-<system>`. Pre-cook the new crates in the `Dockerfile` beside lines 130-132.
- [ ] T004 [P] In `scripts/e2e/slices.json`:
  - add the `sheet-import` slice. It owns `sheet-import-`, and its `standalone` command is `pnpm -F @thunderforge/dnd5e test:sheet-reader` (T043), following `hero-builder`. Its paths are those listed in research R19, and its neighbours are those in the plan's Slice note;
  - remove `specs/048-bringing-a-character-in` from the `hero-builder` summary.
  Then run `node scripts/check-e2e-slices.mjs --fix` so `package.json` gains `e2e:sheet-import` and `e2e:sheet-import:standalone`.
- [X] T005 [P] Declare `feature.sheet_import`:
  - add a `Kind::Bool` in `crates/thunderforge-server/src/settings/registry/declarations.rs` with group "Features", env `THUNDERFORGE_FEATURE_SHEET_IMPORT`, default **false**, `since` the next release, `what_is_limited` "Players cannot bring a character in from a PDF.", and `what_to_set`;
  - add the constant and the `FEATURES` entry in `settings/features.rs`;
  - add the web key in `@/api/featureFlags`;
  - add a commented line in `.env.example`.
- [X] T006 [P] Draft ADR-115, `docs/adrs/20261008-115-character_sheets_are_read_twice_by_one_reader.md` (Nygard, with a Y-statement). It records research R1, R3 and R8. Add its row to `docs/adrs/README.md` with status Proposed.

**Checkpoint**: the workspace builds, the slice exists, and the flag is off.

---

## Phase 2: Foundational (blocks every story)

### The PDF crate (game-agnostic)

- [X] T007 [P] Write `crates/thunderforge-pdf/src/region_tests.rs` with synthetic runs, following `layout_tests.rs`. It covers:
  - `find` matching a label that appears twice;
  - two lines at one y;
  - `right_of` stopping at the next column;
  - `below` within a box;
  - `within`;
  - a rotated page refused as `Page{reason}`.
  The tests fail before T008.
- [X] T008 Implement `crates/thunderforge-pdf/src/region.rs` (`Rect`, `PositionedLine`, `PageText::{read, find, within, right_of, below}`) over the existing `lines()`. Export it from `lib.rs`. T007 goes green.
- [X] T009 [P] Write tests for the bounds and for encryption:
  - `TooLarge` before parsing;
  - `TooManyPages`;
  - `Encrypted`, on a fixture written by lopdf with an `/Encrypt` dictionary;
  - an ordinary file that passes.
- [X] T010 Measure the owner's seven D&D Beyond exports with `cargo run -p thunderforge-pdf --example probe`, from a path outside the repository. Record in research R2 whether any field value is drawn inside a Form XObject. Record counts only, and no values.
- [X] T011 If T010 found values in Form XObjects, follow `Do` into the XObject in `content.rs`, apply its `/Matrix`, and add tests with a synthetic XObject. If T010 found none, record that here and close this task.
  - **Closed by measurement (research R2)**: no value is drawn in a Form XObject, or drawn at all. The exports keep every value in widget `/V`. `Do` traversal is not added; `crates/thunderforge-pdf/src/form.rs` reads the widgets instead, with tests on a synthetic widget document.
- [X] T012 Implement `Limits`, `Document::from_bytes_bounded` and the new `PdfError` variants in `lib.rs`. T009 goes green.
- [X] T013 [P] In `wasm.rs`, add `read_page_text(bytes, page)`. The existing `read_pdf` output must be unchanged, and a test pins it.

### The neutral core

- [X] T014 [P] Write tests in `crates/thunderforge-sheet-import/tests/plan.rs` for every invariant in contracts/sheet-import-core.md:
  - the plan is deterministic, byte for byte;
  - an unread field is never written;
  - an uncertain field is written only when shown;
  - a derived value is never written, and a mismatch becomes a cross-check;
  - every leaf is accounted for, so nothing is dropped;
  - play state is kept on a re-import;
  - `plan_hash` is stable;
  - `normalise_name` and `content_hash`.
  Use hand-built `ImportedCharacter` values with no PDF.
- [X] T015 Implement the types in `character.rs` and `reader.rs`: `ImportedCharacter`, `Field`, `Certainty`, `Source`, `SheetReader`, `Recognition` and `ReadError`.
- [X] T016 Implement `mapping.rs`, which parses the `sheetImport` declaration and validates its paths, and `plan.rs` and `hash.rs`, which hold `plan`, `plan_hash`, `normalise_name` and `content_hash`. T014 goes green.
- [X] T017 In `crates/thunderforge-canvas-core/src/system_contribution.rs`, add the `SheetImport` slot and the object-safe `SheetReaderHandle` (bytes in, JSON out). Add a pack-load test: a malformed `sheetImport` block fails to load, and a pack with no block loads with `sheet_import: None`.

### Storage, origin, permissions, events

- [X] T018 [P] Write server tests in `compendium/origin_tests.rs`:
  - an item or ability origin cannot change;
  - an actor goes from Authored to Uploaded, and never back;
  - an insert with no origin fails;
  - `content_origin()` reads the column for actors, items and abilities.
- [X] T019 Write the migration `2026-10-09-100000-0000_content_origin_columns` (data-model.md §1) with its `down.sql`, and run `diesel print-schema`. Make every `Insertable` for `world_actors`, `world_items` and `world_abilities` state `origin`. The compiler finds them once the field is not `Option`. Make `origin.rs` `origin_of` read the column. T018 goes green. `content-origin.spec.ts` must stay green.
- [X] T020 Write the migration `2026-10-09-110000-0000_sheet_import` (data-model.md §2) and `2026-10-09-120000-0000_unadopted_use_attempts` (§3), with down files. Add the Diesel models in `sheet_import/mod.rs` and `staged_content/mod.rs`.
- [X] T021 [P] Add `require_manages_content(conn, world_id, user_id)` in `auth/world_membership.rs`. It allows the GM and a Trusted Player (`manages_content()`) and refuses a Player. Add tests.
- [X] T022 [P] Add `sheet_import/storage.rs` with `object_key(owner, character, version)` under `sheets/`, following `feedback/mod.rs`. Allow `sheets/` in `delete_object` (`rustfs.rs:477`). Test that a `sheets/` key deletes and any other prefix is still refused.
- [X] T023 [P] Add world event codes 40 `SHEET_IMPORT_APPLIED`, 41 `STAGED_CONTENT_DECIDED` and 42 `ACTOR_ROLLED_BACK` in `world_events.rs` (39 went to spec 088). In the web event sync, refetch the actor's sheet and links on 40 and 42, and the staged queue on 41.

### Fixtures

- [X] T024 Write the fixture generator `packs/systems/dnd5e/sheet/tests/fixtures/gen.rs`.
  - It uses lopdf's writer, has no timestamps, and sets a fixed `/ID`.
  - It writes the eight fixtures in contracts/sheet-mapping-5e.md. The layout's labels and positions come from the owner's exports, and every value is invented.
  - A test asserts each generated file's sha256, so a generator change is a visible diff.
- [X] T025 [P] Add `packs/systems/dnd5e/sheet/examples/measure_corpus.rs`. No test runs it. It reads `THUNDERFORGE_SHEET_CORPUS` and exits with a note when the variable is unset. For each file it prints the read, uncertain and unread counts per field, and no value. Add `sheet-corpus/` to `.gitignore`.

**Checkpoint**: `cargo test -p thunderforge-pdf -p thunderforge-sheet-import -p thunderforge-canvas-core` is green, the migrations run up and down, and the fixtures generate the same bytes twice.

---

## Phase 3: User Story 1 - A player brings their own character (P1) MVP

**Goal**: from the actor screen, a player imports a D&D Beyond PDF onto the
5e character they hold, reviews what was read, and accepts or declines it.
Content the world lacks is staged under their name and kept out of play.

**Independent test**: `sheet-import-player.spec.ts`.

### The 5e pack holds the whole sheet (Q1=C)

- [X] T026 [P] [US1] Write validator tests in `packs/systems/dnd5e/server/src/validators.rs`'s test module for every row in data-model.md's 5e table. Examples: `level` must equal the sum of `classes`, a pool's `used` must not exceed its `total`, coins cannot be negative, and an unknown damage type is refused.
- [X] T027 [US1] Grow `packs/systems/dnd5e/system.json`:
  - the `trait_data`, `resource_data` and `spell_data` fields;
  - a `damageTypes` list;
  - the vocabulary types `feature` and `species_trait`.
  Then implement the validators. T026 goes green. Delete the unused `server/src/models.rs:13-48`.
- [X] T028 [P] [US1] Add the pack's web sheet sections in `packs/systems/dnd5e/web/src/sheet/`: `ClassesSection.tsx`, `DefencesSection.tsx`, `PersonaSection.tsx` and `CoinsSection.tsx`. Add `LinkedContent.tsx`, which lists linked spells, features and items and marks staged ones "awaiting the GM" or "declined by the GM". Mount them in `ActorSheet.tsx`, with mount points only. Add vitest tests for each section.
  - Done: the four sections mount as regions in `sheet-regions.ts`. `LinkedContent.tsx` and its test are in place; it mounts in T039, because no read gives the sheet a link's staged state until T036.

### The reader (TDD)

- [X] T029 [P] [US1] Write tests in `packs/systems/dnd5e/sheet/tests/read.rs` against `fighter-5`, `fighter3-wizard2`, `cleric-7` and `rogue-4`:
  - recognition;
  - every table row's value and certainty;
  - the spell, feature, equipment and attack rows;
  - a non-caster with no spell page.
  These fail before T030.
- [X] T030 [US1] Implement `recognise.rs` (the anchors) and `fields.rs` (label-relative regions built on `region.rs`). Add `glyphs.rs`, the proficiency mark table. Each `Uncertain` carries a reason a player can act on.
  Deviation: `fields.rs` reads each value by its form-field name, not by label-relative region, because every value in a D&D Beyond export sits in a named widget field (T011, closed by measurement). The labels are still the recognition anchors.
- [X] T031 [US1] Implement `content.rs`, which reads the spell rows (with prepared marks and level), the FEATURES & TRAITS headings with uses and recharge, the equipment (with quantity, weight, and equipped and attuned marks), and the attack rows. T029 goes green.
- [X] T032 [US1] Implement `wasm.rs` (`readSheet(bytes) -> {recognised, reading | error}`). Export `sheetReader: () => import("@thunderforge/sheet-dnd5e")` from `packs/systems/dnd5e/web/src/index.ts`. Add `apps/web/src/pages/world/actor/systemSheetReaders.ts`, globbed like `systemActorSheets.ts`. `scripts/check-system-registry.mjs` stays green.

### The mapping (TDD)

- [X] T033 [P] [US1] Write tests in `packs/systems/dnd5e/server/src/sheet_import_tests.rs` that plan each fixture's reading onto an empty 5e actor. Check every target in the field table, and the refine rules: the level sum, the hit-dice string, an attack as an item when the weapon is in the equipment and as an ability otherwise, and a spell shared by two classes.
- [X] T034 [US1] Add the `sheetImport` block to `system.json` (contracts/sheet-mapping-5e.md). Implement `refine` in `server/src/sheet_import.rs` and register the `SheetImport` slot with the native reader in `server/src/lib.rs`. T033 goes green.

### The server

- [X] T035 [P] [US1] Write `crates/thunderforge-server/src/graphql/mutations_sheet_import_tests.rs`:
  - the preview writes nothing;
  - an apply writes the fields, links, version, record, origin `Uploaded` and event 40 in one transaction;
  - a player whose claim granted Editor may import;
  - the GM may import onto any actor in the world;
  - a Viewer and a stranger are refused with `FORBIDDEN`;
  - the flag off is refused with `FEATURE_DISABLED`;
  - a changed plan is refused with `PLAN_CHANGED` and writes nothing;
  - a failed transaction deletes the object;
  - a decline uploads nothing.
- [X] T036 [US1] Implement `sheet_import/preview.rs`, `apply.rs`, `snapshot.rs` and the `SheetImportMutation` and `SheetImportQuery` (`sheetImportPreview`, `applySheetImport`, `actorImports`) per contracts/graphql-sheet-import.md. Merge them into `graphql.rs`. Run `node scripts/check-graphql-contract.mjs --schema --fix`.
- [X] T037 [US1] In `apply.rs`, upsert `world_staged_content` by its unique key and write links with `staged_id`. Reuse world content by kind and normalised name (FR-031). Write a test that a staged link is absent from every play read: `play_field.rs`, `sheet.rs`, the combat panel, the roll buttons' query and the world compendium. T035 goes green.
  - Done: `play_field.rs` and `sheet.rs` hold no link reads (the claim registry and the sheet declarations). The play reads are `actorAbilities` and `actorInventory` (the roll buttons and the combat menu), `find_weapon` (attacks), and `worldAbilities`/`worldItems` (the compendium). The first two now leave out a link with `staged_id`; the rest never see one, because they join by world id. The test covers all five, for the player and the GM.

### The web

- [X] T038 [P] [US1] Add `apps/web/src/api/sheetImport.ts`, the typed calls, with `applySheetImport` sent through `postGraphQLMultipart` with progress. Add the hook `useSheetImport(actorId)` with `refetch()`.
  - Done: `postGraphQLMultipart` sends with `XMLHttpRequest` when `onUploadProgress` is given, because `fetch` cannot report upload progress. It has three tests.
- [X] T039 [US1] Add the route `/world/:id/actor/:actorId/import`, loaded with `React.lazy`, to `apps/web/src/pages/world/actor/import/SheetImportPage.tsx`. The page:
  - picks a file;
  - reads it with the pack's lazy reader;
  - asks for the preview;
  - shows `FieldRow.tsx` (certainty, old and new values, source text) and `ContentRow.tsx` (world, staged or new);
  - ends in Accept and Decline.
  Add vitest tests for the rows.
  - Done: the page offers Accept and Decline only; corrections and play-state overwrites come with T049. Contract extension: the query `actorStagedLinks(actorId)` (types `ActorStagedLink`, `StagedState`), which the 5e sheet's "Brought in with a sheet" section reads to mount `LinkedContent.tsx`, since `actorAbilities` and `actorInventory` withhold staged links. It and `actorImports` now require a seat at the table (`require_sees_actor`): the permission ladder admits a stranger at Viewer.
- [X] T040 [US1] Add "Bring in a sheet" to the header button group of `ActorDetailPage.tsx`, at about line 405. It appears when `mayEditActor` holds, `useFeatureFlag("feature.sheet_import")` is on, and the system declares `sheetImport`. Mount point only.
- [X] T041 [P] [US1] Add the entry `player-hero-import-${actorId}` to the player's own hero row on `PlayersPage.tsx`, beside the hotfix's `player-hero-sheet-${actorId}`. It needs the hotfix merged.
- [ ] T042 [US1] Write `apps/web/e2e/sheet-import-player.spec.ts`, with the flag on for the test stack:
  - a player claims a 5e actor and opens it from the Players screen (the hotfix's path);
  - they bring in `fighter3-wizard2.pdf`, review it and accept;
  - the sheet shows both classes, level 5, the scores, hit points, proficiencies, spells and coins;
  - a feat the world lacks shows "awaiting the GM" and is absent from the play field;
  - a second run that declines writes nothing;
  - the GM imports onto an NPC;
  - a second player sees no button, and the server refuses a direct call.
- [X] T043 [P] [US1] Add a harness page and a Playwright config to `packs/systems/dnd5e/web` (script `test:sheet-reader`), with `e2e/sheet-reader.spec.ts`. This is the slice's standalone half. With no server, it reads every fixture in the browser and checks that the wasm reading equals the native reading, which `cargo test` writes to `target/sheet-fixtures/*.json`.
  - Done: `tests/browser.rs` writes each answer exactly as `readSheet` returns it (parsing it into a `serde_json::Value` first rounds the f32 rectangles), and `test:sheet-reader` runs that test before the build, so the comparison never reads stale answers. The harness is `sheet-reader/` and `vite.sheet-reader.config.ts`, on port 5196. 9 passed: one per fixture, plus the count.
- [ ] T044 [US1] **Proof**: `pnpm e2e:sheet-import` and `pnpm e2e:sheet-import:standalone` are green. Then run every slice that `pnpm e2e:which --diff` names. The schema changed, so it prints FULL SUITE; run the named slices instead, which must include `actors` (with `players-hero-edit.spec.ts`), `collections`, `compendium` and `combat`. Never run `node ./scripts/e2e-parallel.mjs` on its own. Record each slice's result here.

**Checkpoint (MVP)**: a player brings a D&D Beyond character onto the actor
they hold, from the actor screen, and nothing is written without their
accept.

---

## Phase 4: User Story 2 - Nothing is guessed in silence (P1)

**Goal**: every field says read, uncertain (and why) or unread. The person
can correct what is uncertain, values the pack derives are cross-checked,
and what cannot be read is refused whole, with a reason.

**Independent test**: `sheet-import-review.spec.ts` and `sheet-import-refusals.spec.ts`.

- [X] T045 [P] [US2] Write reader and plan tests:
  - `uncertain-mark.pdf` gives an Uncertain field with its reason;
  - a printed Perception that disagrees with the pack's derivation becomes a cross-check showing both numbers, and the derived value is not written;
  - an unread field stays as it was;
  - a correction is written and marked `CORRECTED`;
  - `warforged-defences.pdf` sends disease immunity to unmapped notes.
- [X] T046 [US2] Make T045 pass in `fields.rs` and the core `plan.rs`. Have `refine` append unmapped values to `trait_data.notes` under "From the imported sheet" (FR-013).
  - Done: the uncertain, unread, corrected and cross-check rules were already in the core `plan.rs` (T035); the new rule is the pack's Perception cross-check in `packs/systems/dnd5e/server/src/sheet_import.rs` (Wisdom's modifier plus the mark's share of the bonus: half for Jack of All Trades, double for expertise), against the skill marks. The warforged fixture now prints "Disease - Immunity" in its Defenses box, where D&D Beyond prints it; the reader's unknown-defence path makes it a "Defenses" note, and it plans as unmapped to `trait_data.notes` (fixture re-pinned). The notes block is written by `notes_with_unmapped` in `sheet_import/apply.rs`, now tested in `apply_tests.rs`. On the owner's 7 real exports the plan raises no cross-checks.
- [X] T047 [P] [US2] Write server tests for each refusal code in contracts/graphql-sheet-import.md: `SHEET_ENCRYPTED`, `SHEET_TOO_LARGE` (before the body is parsed), `SHEET_TOO_MANY_PAGES`, `SHEET_UNREADABLE`, `SHEET_NOT_RECOGNISED` (`not-a-ddb-sheet.pdf`) and `SYSTEM_HAS_NO_MAPPING` (a pack with no block). Each writes nothing.
- [X] T048 [US2] Map the `PdfError` and `ReadError` values to those codes in `sheet_import/mod.rs`, with the sentence a player reads. T047 goes green.
  - Done: the mapping lives on `ReadError` in the core crate (`code()` and `sentence()`, tested in `crates/thunderforge-sheet-import/tests/refusals.rs`), so the pack's server reader, the server's own size check in `apply.rs` and the browser's reader answer with the same code and sentence. The browser's reader now opens the file within the server's bounds and answers `code` beside `error`. Two generated fixtures join the corpus: `password-protected.pdf` and `too-many-pages.pdf` (21 pages). `SHEET_UNREADABLE` is asked with bytes that are not a PDF; `SYSTEM_HAS_NO_MAPPING` with a Roll for Shoes actor, through the preview and the apply.
- [X] T049 [US2] Grow the review in `SheetImportPage.tsx`:
  - filters for uncertain and unread;
  - a correction input on those rows;
  - cross-checks that show the sheet's number and the system's;
  - a "will overwrite" marker on every changed field (FR-021);
  - the unmapped list and where each value goes.
  Show the same refusal sentences in the browser before any upload.
  - Done: filters All / Check this / Not read; a correction input on every row the reader was unsure of (uncertain, unread, corrected), which asks for the plan again with the corrections, so the server marks the row CORRECTED and the hash covers it; a "will overwrite" or "new" marker on every changed row; the cross-checks with both numbers; on a re-import, a box per value in play to take the sheet's (`overwritePlayState`); the unmapped list with where each value goes. The browser shows the reader's refusal sentence with the server's code (`data-code`) before any upload. Contract extension: `SheetImportPlan.crossChecks [SheetCrossCheck]` and `keptInPlay [SheetKeptInPlay]`, which the plan already carried and hashed but GraphQL did not expose. Refusal wording moved to `import/refusal.ts`.
- [X] T050 [US2] Store the player's corrections apart from the server's reading on `sheet_import_versions`. Expose `correctedFields` on `ActorImportRecord`, so the GM sees what the player changed (FR-022). Add a test.
  - Done: already stored by T037 (`sheet_import_versions.corrections`, apart from `reading`) and exposed by T039 (`correctedFields`). The test `a_correction_is_kept_apart_from_the_reading_and_the_gm_sees_it` proves it end to end at the server: the actor gets 15, the version keeps the correction and the reading's 13, and the GM's `actorImports` names `abilities.str`.
- [ ] T051 [P] [US2] Write `apps/web/e2e/sheet-import-review.spec.ts`: the player corrects an uncertain mark, sees a cross-check, accepts, and the GM sees which field was corrected.
- [ ] T052 [P] [US2] Write `apps/web/e2e/sheet-import-refusals.spec.ts`: an encrypted file, an oversized file, a book PDF, and an actor whose system declares no mapping (the button is absent, and a direct call is refused). Nothing is written in any of them.
- [ ] T053 [US2] **Proof**: run `pnpm e2e:sheet-import`, then the slices `pnpm e2e:which --diff` names, never the full suite. Record the results here.

**Checkpoint**: no value lands that the person was not shown (SC-003).

---

## Phase 5: User Story 3 - The world learns from a character (P1)

**Goal**: the GM or a Trusted Player sees what each player brought, grouped
by player, and adopts, adopts all, declines or revisits. Unadopted content is
refused in play and reported to the GM, but not when the client is merely
stale.

**Independent test**: `sheet-import-adopt.spec.ts` and `sheet-import-unadopted.spec.ts`.

- [X] T054 [P] [US3] Write `graphql/mutations_staged_content_tests.rs`:
  - an adopt creates one `world_abilities` row with origin `Uploaded`, repoints every link and leaves no duplicate (FR-034);
  - adopt all takes a snapshot, so a piece staged afterwards stays pending;
  - decline and revisit;
  - a Player is refused, including on their own content;
  - a Trusted Player is allowed;
  - the same piece from two characters is one row;
  - the same name with different content is two rows with `differs_from`;
  - each decision records event 41.
  - Done: 10 tests, all green. The last one applies `fighter3-wizard2.pdf` onto two actors as the GM and checks that each piece is staged once and one adoption serves both. Each decision test reads event 41 back by `stagedId`.
- [X] T055 [US3] Implement `staged_content/decide.rs` and `StagedContentMutation` (`adoptStagedContent`, `adoptAllStagedContent`, `declineStagedContent`, `revisitStagedContent`), and the `stagedContent` query. Each checks `require_manages_content`. T054 goes green.
  - Done: the rules are in `decide.rs`, and the resolvers only convert. `StagedContentQuery` sits beside the mutations in `mutations_staged_content.rs`, not in `queries/sheet_import.rs`. `StagedContent` also carries `adoptedAbilityId` and `adoptedItemId`. `ActorSummary` is `{id, label}`. The contract records both. An adopted item becomes a `world_items` row; anything else becomes a `world_abilities` row whose classification is the kind. Declining an adopted piece is refused with `VALIDATION_FAILED`. Revisit starts only from declined. A stranger gets `FORBIDDEN`. `--lib staged_content` gives 10/10, `--lib sheet_import` gives 30/30, and clippy is clean on the server, sheet-import and both dnd5e crates.
- [X] T056 [P] [US3] Write refusal tests: `rollCheck`, `makeAttack`, ability use, item use and share each refuse a staged link with `CONTENT_NOT_ADOPTED` and the FR-036a sentence.
  - Done: there are 6 tests in `staged_content/guard_tests.rs`.
    - `makeAttack` with a staged ability or item refuses with the code and the sentence. A declined piece is refused too.
    - An adopted piece's staged id gives the ordinary not-found, and the world's id works.
    - Spending a staged item through `adjustInventoryQuantity` is refused and leaves the quantity unchanged. This is "item use".
    - Both share links are refused for the bringer, the GM and a Trusted Player.
    - Another player, a stranger, or a creature that does not hold the piece gets the path's ordinary answer, so the refusal never reveals a piece.
    - Deviation: `rollCheck` has no test. It takes a check the system declared and never names an ability or item, so it cannot name a piece. Ability use has no test either, because there is no ability-use mutation yet: an ability is used through `makeAttack`, which is covered. Both are recorded in the guard's module doc.
- [X] T057 [US3] Implement `staged_content/guard.rs`, one check called from each of those mutations. T056 goes green.
  - Done: `guard.rs` has three lookups, one for each way a path can name a piece (an attacker's link, an inventory entry, a share), plus `Unadopted::refusal()`.
    - `find_weapon` asks the guard only after its own lookup misses, so the lair, the reroll and the preview are all covered. `FightRefusal::NotAdopted` carries the piece for T058's report.
    - The offline reconcile path rejects a not-adopted attack as `PermissionDenied`.
    - The share check runs before the readiness gate, because it answers only people who can already see the piece.
    - Test counts: `--lib staged_content` 16/16, combat 126/126, inventory 5/5, ability shares 10/10, item shares 8/8, reconcile 26/26. Clippy is clean.
- [ ] T058 [P] [US3] Write report tests in `staged_content/report_tests.rs`:
  - the first attempt is recorded and posts one GM-only chat message stating facts only;
  - a second attempt within 10 minutes is counted and not posted;
  - a header older than the declining event is suppressed;
  - a missing header is suppressed;
  - a piece never delivered (always pending) is always reported.
- [ ] T059 [US3] Implement `staged_content/report.rs`. Send `x-tf-last-event` from `apps/web/src/api/graphqlClient.ts`, using the world store's last applied event id. T058 goes green.
- [ ] T060 [US3] Add `apps/web/src/components/world/staged/BroughtByPlayers.tsx` and `StagedRow.tsx`, the GM and Trusted Player queue. They are grouped under one heading per player, with Adopt, Adopt all, Decline and Revisit, and show where a piece differs from another character's. Mount the queue as a "Brought by players" tab on the world compendium screen, behind `manages_content`. Add vitest tests.
- [ ] T061 [P] [US3] Show the refusal sentence when a player's action names staged content. The marks in `LinkedContent.tsx` follow event 41.
- [ ] T062 [US3] Write `apps/web/e2e/sheet-import-adopt.spec.ts`:
  - a GM adopts a feat in one action, and it appears in the world compendium (SC-005);
  - a second player's character that brought the same feat now uses the world's;
  - a Trusted Player adopts all of one player's content;
  - a Player sees no adopt control, and the server refuses a direct call.
- [ ] T063 [US3] Write `apps/web/e2e/sheet-import-unadopted.spec.ts`:
  - a declined spell is absent from the play field and the combat panel;
  - a forced call is refused, and the GM's chat shows the report;
  - a client held stale across the decline is refused and not reported.
- [ ] T064 [P] [US3] Write `apps/web/e2e/sheet-import-attack.spec.ts`, which is the combat neighbour: an imported weapon attack, once adopted, is made in a fight through `makeAttack`.
- [ ] T065 [US3] **Proof**: run `pnpm e2e:sheet-import`, then the slices `pnpm e2e:which --diff` names, which must include `compendium` and `combat`. Never run the full suite. Record the results here.

**Checkpoint**: unadopted content never reaches the play field (FR-037),
and nobody is accused for a stale screen (FR-038b).

---

## Phase 6: User Story 4 - One importer, many systems (P2)

**Goal**: a second system imports through the same pipeline, with no change
outside its own pack (SC-006).

**Independent test**: `sheet-import-second-system.spec.ts`.

- [ ] T066 [US4] Confirm open item 2's default: Roll for Shoes with a one-page ThunderForge sheet. If the owner names another system, change the paths in T067-T070.
- [ ] T067 [P] [US4] Create `packs/systems/roll_for_shoes/sheet/` (`thunderforge-system-roll-for-shoes-sheet`) with a fixture generator for the one-page sheet (name, skills with levels, XP) and reader tests, test-first.
- [ ] T068 [US4] Implement that reader, the `sheetImport` block in `packs/systems/roll_for_shoes/system.json` (into `trait_data.skills` and `resource_data.xp`), the slot registration in its `server/src/lib.rs`, and the `sheetReader` export in its `web/src/index.ts`.
- [ ] T069 [US4] Write `apps/web/e2e/sheet-import-second-system.spec.ts`: a Roll for Shoes player brings in the fixture through the same review screen.
- [ ] T070 [US4] **Proof**: `git diff --stat` for T067-T069 touches only `packs/systems/roll_for_shoes/**`, the new e2e spec, `Cargo.toml`'s member list and `slices.json`. Record that here (SC-006). Run `pnpm e2e:sheet-import`, then the slices `pnpm e2e:which --diff` names, never the full suite.

**Checkpoint**: the next system is a mapping, not a second importer.

---

## Phase 7: User Story 5 - A character comes back changed (P3)

**Goal**: a re-import shows only what differs and keeps play state. The GM
can roll back to any version, and every kept file is reachable only by its
owner and the GM.

**Independent test**: `sheet-import-reimport.spec.ts` and `sheet-import-rollback.spec.ts`.

- [ ] T071 [P] [US5] Write plan and server tests for a re-import:
  - `fighter3-wizard2` to `fighter3-wizard2-l6` changes only the differing fields;
  - current HP, used slots and death saves are kept unless `overwritePlayState` names them (FR-051);
  - content adopted since the first import uses the world's version;
  - content no longer on the sheet is `removed: true` and is unlinked only when accepted.
- [ ] T072 [US5] Implement the re-import plan (`isReimport`, `removed`, `keptInPlay`) and reuse `brought_characters` with the next `version_no`. T071 goes green.
- [ ] T073 [US5] Add a diff mode to the review: only the differences, with a tick per play-state field to overwrite it.
- [ ] T074 [P] [US5] Write `rollBackActor` tests:
  - the GM only, with a Trusted Player and the owner refused (FR-044b);
  - the sheet fields and links are restored;
  - play state is kept (FR-044a);
  - a `rollback` record and event 42 are written;
  - origin stays `Uploaded`;
  - rolling back to "before any import" restores the first snapshot.
- [ ] T075 [US5] Implement `sheet_import/rollback.rs` and `rollBackActor`. T074 goes green.
- [ ] T076 [P] [US5] Write tests for the file route. The owner gets 200. The GM of a world where the version was applied gets 200. A Trusted Player, another world's GM and a stranger each get 404. Check `Cache-Control: private, no-store`.
- [ ] T077 [US5] Implement `GET /api/sheet-imports/{versionId}/file` in `sheet_import/route.rs` and register it with the server's routes. T076 goes green.
- [ ] T078 [US5] Add `apps/web/src/pages/world/actor/import/ImportHistory.tsx` to the actor screen. It lists versions and rollbacks with who and when. Download appears for the owner and the GM, and Roll back for the GM only.
- [ ] T079 [P] [US5] Write `apps/web/e2e/sheet-import-reimport.spec.ts`: level 5 then level 6, where the review shows only the differences and the table's current HP survives.
- [ ] T080 [P] [US5] Write `apps/web/e2e/sheet-import-rollback.spec.ts`:
  - the GM rolls back a bad import and play state survives;
  - the history reads import, import, rollback;
  - the player cannot roll back;
  - the player downloads their file, and another player cannot.
- [ ] T081 [US5] **Proof**: run `pnpm e2e:sheet-import`, then the slices `pnpm e2e:which --diff` names, never the full suite. Record the results here.

**Checkpoint**: sheets change every session, and the importer keeps up
without throwing play away.

---

## Phase 8: Polish and cross-cutting

### The account's data

- [ ] T082 [P] Write export tests: `export_user_data_payload` (`users/mod.rs:187`) carries brought characters, versions and import records at manifest v4, and `build_zip_export` (:502) adds each kept file under `sheets/`.
- [ ] T083 Implement T082 in `users/mod.rs` and `users/export_content.rs`.
- [ ] T084 [P] Write deletion tests:
  - `delete_user_data_on` (:406) removes the rows in its transaction;
  - the objects are deleted after commit, following `feedback/schedule.rs:179`;
  - a rescued character (`collections/rescue.rs:47`) keeps its import records, marked "file no longer kept" (open item 1's default).
- [ ] T085 Implement T084.
- [ ] T086 [P] Extend `apps/web/e2e/user-data-export.spec.ts` (the `accounts` slice) and `library-account-deletion.spec.ts` (the `collections` slice) with an imported sheet.

### Telemetry, coordinated with spec 086

- [ ] T087 Instrument the five server points in research R16 at the places they occur: `apply.rs` outcomes, the read duration, the field certainties, `decide.rs` and `report.rs`.
  - If 086's `crates/thunderforge-telemetry-policy` exists, add the names to `INSTRUMENTS` and to its contract table in `specs/086-full-telemetry/contracts/server-instruments.md`, and record the instruments with `opentelemetry::global::meter("thunderforge")`.
  - If it does not, emit the same points as `tracing` spans with those field names, and add a row to 086's contract so its tasks pick them up.
  - Add a test that the attributes are only the closed sets.
- [ ] T088 [P] Add the browser event `sheet_import.step` (`step`, and `reason` on `failed` only) at the review's steps.
  - If `packages/telemetry` exists, use it: the `EventName` type, `ALLOWED_ATTRIBUTES` and its test.
  - If it does not, add a row to 086's `contracts/browser-events.md` events table and a no-op call site.
  - Tests run with `TELEMETRY=false`.

### The flag, the docs, the record

- [ ] T089 [P] Write `apps/web/e2e/sheet-import-flag.spec.ts`, following `instance-feature-flags.spec.ts`. With the flag off, the button is absent and a direct call gets `FEATURE_DISABLED`, while staged decisions and downloads still work. With it on, the import works.
- [ ] T090 [P] Write the user guide `docs/guides/bringing-a-character-in.md`. It covers:
  - what a player does from the actor screen and the Players screen;
  - reading the review (read, uncertain, unread, cross-checks);
  - what "awaiting the GM" and "declined by the GM" mean;
  - for the GM and Trusted Players, the "Brought by players" queue and adopt all;
  - re-import and rollback;
  - where the file is kept and who can download it.
  Link it from `docs/guides/characters-for-your-players.md`.
- [ ] T091 [P] In `docs/CONTRIBUTING.md`, add a "Bringing a character in" section. It covers:
  - the reader trait and the `SheetImport` slot;
  - the `sheetImport` block;
  - the read-twice rule and the plan hash;
  - the staged table, and the rule that play joins only adopted ids;
  - the fixture policy: generated, deterministic, nobody's sheet;
  - `measure_corpus`;
  - how to add a system's reader.
- [ ] T092 Move ADR-115 to Accepted, with what was built. Update the README row.
- [ ] T093 Run `measure_corpus` on the owner's seven exports. Record per-field read, uncertain and unread counts in research R17, with no values, for SC-002. Each field the corpus reads wrong becomes a reader fix and a fixture case, before T097.
- [ ] T094 [P] Add `apps/web/playtest/bring-a-character.playtest.ts` (FR-061). A player brings `cleric-7.pdf` in from the actor screen, the GM adopts their domain spell, and the character casts it at the table. It runs with `pnpm playtest`, last.

### Verify and prove

- [ ] T095 Run `make lint` (host and wasm32, plus the file-length check), `pnpm verify`, `pnpm -F @thunderforge/web exec tsc --noEmit`, `node scripts/check-graphql-contract.mjs --schema` and `node scripts/check-system-registry.mjs`. All must be green.
- [ ] T096 Run the crate and unit tests. All must be green.
  - `cargo test -p thunderforge-pdf -p thunderforge-sheet-import -p thunderforge-system-dnd5e-sheet -p thunderforge-system-dnd5e -p thunderforge-system-roll-for-shoes-sheet`;
  - `make test-rust ARGS="-p thunderforge-server"`. It needs the `thunderforge-canvas-assets` bucket;
  - `pnpm -F @thunderforge/web test`.
- [ ] T097 Flip `feature.sheet_import` to default **true** in `declarations.rs` and `.env.example`, and set `since`.
- [ ] T098 **Proof**: `pnpm e2e:sheet-import` and `pnpm e2e:sheet-import:standalone` are green. Then run every slice that `pnpm e2e:which --diff` names. It prints FULL SUITE because of the migrations and schema; run the named slices instead (at least `actors`, `compendium`, `combat`, `book-import`, `collections`, `accounts` and `instance`). Never run `node ./scripts/e2e-parallel.mjs` on its own. Then run `pnpm playtest`. Record each result here.

---

## Dependencies

- Phase 1 comes before everything else. Phase 2 blocks every story.
- **Phase 3 waits on `hotfix-player-hero-edit` merging into `main`.** It
  also needs T008, T016, T017, T019, T020 and T024.
- US2 (Phase 4) builds on US1's reader, plan and review screen.
- US3 (Phase 5) builds on US1's staged links (T037). It can run beside US2
  once T037 lands.
- US4 (Phase 6) needs only Phase 2 and T036. It can run beside US2 and US3.
- US5 (Phase 7) needs US1. Its rollback (T075) needs the before-snapshot
  from T036.
- The Phase 8 account tasks need T020. T097 and T098 come last.

## Parallel opportunities

- **Phase 2**:
  - the PDF track (T007-T013), the core track (T014-T016) and the server
    track (T018-T023) touch different crates;
  - T024 and T025 can run beside all three.
- **Phase 3**: the pack sheet sections (T028), the reader tests (T029), the
  mapping tests (T033) and the server tests (T035) are in different files.
- **US3**: the guard (T056-T057) and the report (T058-T059) can run beside
  the decisions (T054-T055).
- **US4** runs on its own pack.

## Implementation strategy

1. **MVP**: Phases 1-3. A player brings a D&D Beyond character in from the
   actor screen, reviews it and applies it. Unknown content is staged and
   kept out of play. The proof is `sheet-import-player.spec.ts`.
2. Add US2, so nothing is silent and refusals are whole.
3. Add US3, so the world adopts content and unadopted content is refused and
   reported.
4. Add US4, Roll for Shoes as the second mapping.
5. Add US5, re-import and rollback, with the kept files reachable.
6. Finish with Phase 8. The flag turns on with the proof. The merge gate is
   `pnpm e2e:sheet-import`, plus every slice `pnpm e2e:which --diff` names,
   plus `pnpm playtest`. It is never the full suite.
