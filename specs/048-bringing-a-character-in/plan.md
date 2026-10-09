# Implementation Plan: Bringing a Character In

**Branch**: `048-bringing-a-character-in` | **Date**: 2026-10-08 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `specs/048-bringing-a-character-in/spec.md`

## Summary

A player opens their character's actor screen, chooses "Bring in a sheet"
and picks their D&D Beyond PDF export.

1. The browser reads the export with the 5e pack's reader, compiled to wasm.
2. The server maps the reading and shows the player every field: what was
   read, what was uncertain and why, what is unread, and what will be
   overwritten.
3. On accept, the file is uploaded. The server reads it again with the same
   reader, checks that the plan matches the one the player reviewed, and
   applies it in one transaction.
4. Content the world does not have arrives staged under the player's name.
   The GM or a Trusted Player adopts it, declines it, or adopts everything
   one player brought. Unadopted content never reaches the play field, and
   an attempt to use it is refused and reported to the GM.
5. Each import is a version with its file kept, and the GM can roll an actor
   back to any of them.

All three owner questions are answered (spec, Decisions):

- **Q1 = C**: the 5e pack holds the whole sheet;
- **Q2 = B, versioned**: the file is kept with each import, and the GM can
  roll back;
- **Q3 = C, and stronger**: unadopted content is withheld from play, refused
  and reported. A Trusted Player may adopt (decision 4).

By layer:

- **`thunderforge-pdf`** (stays game-agnostic):
  - reused: the parse, the repair, the text runs, the fonts and the
    ToUnicode CMaps, line layout, and the wasm build;
  - new: `region.rs` for positioned label and region queries; byte and page
    bounds; an `Encrypted` refusal; Form XObject traversal, only if the
    owner's exports turn out to need it (R2).
- **`thunderforge-sheet-import`** (new crate, names no system):
  - the neutral `ImportedCharacter`, with a certainty and a source on every
    field;
  - the `SheetReader` trait;
  - the pure mapping engine that turns a reading, the actor and the world
    into an `ImportPlan` with a hash (R3).
- **Canvas core**: a `SheetImport` slot on `SystemContribution` (R5,
  contracts/sheet-import-core.md).
- **5e pack**:
  - the D&D Beyond reader, as a new crate in `packs/systems/dnd5e/sheet/`
    (native and wasm, R4);
  - a `sheetImport` declaration in `system.json` and a `refine` hook;
  - the sheet grows to the whole character (Q1=C, R6): classes and levels,
    hit-dice pools, coins, defences, appearance and personality, multiclass
    spellcasting, pact slots, the `feature` and `species_trait` vocabulary
    types, a `damageTypes` list, and sheet sections for linked content.
- **Server**:
  - three migrations (data-model.md):
    - an `origin` column on actors, items and abilities, with no default
      (FR-033d);
    - the versions, import records and staged content tables, plus link
      columns;
    - the unadopted-use reports;
  - `sheetImportPreview`, `applySheetImport`, `rollBackActor`, and the
    staged-content decisions (contracts/graphql-sheet-import.md);
  - kept files in RustFS under `sheets/`, served only to the owner and the
    GM;
  - `CONTENT_NOT_ADOPTED` refusals in play mutations, with GM reports
    suppressed for a stale client (R13);
  - world events 39-41;
  - the export and account deletion paths cover the new rows and files
    (R12);
  - `feature.sheet_import` (R15).
- **Web**:
  - the button on the actor screen and on the Players-screen row for the
    player's own hero;
  - a lazy review route;
  - the GM's "Brought by players" queue, grouped by player;
  - an import history with download and GM rollback;
  - the `x-tf-last-event` header.
- **Roll for Shoes**: the second mapping, which proves SC-006 by changing
  nothing outside its own pack (US4, open item 2).

### Builds on

- **Book import** (spec 049): the browser reads with wasm, the server
  re-checks, and `contentPatterns` with `refine_content` is the precedent for
  a pack declaration plus a pack hook. The review screen borrows
  `BookReview.tsx`'s row pattern but not its component.
- **Actor permissions**: since ADR-110, a claim grants Editor, so the player
  who claimed the actor may import onto it. The GM is always Owner.
  `require_actor_permission` is the check.
- **Commit `283f4143`** added the Trusted Player (`Role::TrustedPlayer`,
  `manages_content()`), and the plan for staging what players bring under
  their name.
- **Commit `a56fe3fc`** fixed what "uploaded" means: a book or a sheet, not
  a map. The origin column uses that meaning.
- **RustFS**, self-hosted first: the existing bucket, a new `sheets/`
  prefix, and the deletion model of `feedback/` (R11).
- **The `hotfix-player-hero-edit` branch** (`ca568bff`, `cc28cd0e`, not yet
  on `main`) gives a player the way from the Players screen to their own
  hero's sheet, and an editable 5e sheet. **Phase 3 starts after it
  merges.** Phases 1 and 2 do not depend on it.

### What is deliberately not built

- **Taking a brought character to a second world from the account.** The
  rows support it (FR-030b), and a later spec adds the screen. Until then, a
  player imports the same file into the other world, and
  `brought_characters` is reused when the owner matches.
- **Creating an actor from a sheet.** An import goes onto an actor the
  player already holds. A new character uses the existing create-and-claim
  path first, so an actor is never staged.
- **Readers for other publishers' exports**, such as a fillable WotC sheet,
  Foundry JSON or a Pathfinder sheet. The trait and the declaration make
  each one a pack change (US4).
- **Decrypting a password-protected PDF.** It is refused with a reason.
- **OCR of a scanned sheet.** A sheet that draws no text is refused as
  unreadable.
- **An AI reading or "fixing" a sheet.** The reader is deterministic. Every
  uncertainty goes to a person, and nothing is guessed.
- **Withdrawing adopted content.** That is ordinary compendium deletion.
- **Per-world feature switches.** Flags are instance-wide.

## Technical Context

**Language/Version**: Rust 2024 for the PDF crate, the new
`thunderforge-sheet-import`, canvas core and the server. The 5e pack's server
crate stays on edition 2021, and the new `thunderforge-system-dnd5e-sheet`
uses 2024. TypeScript 5 with React 19 for the web and the pack's web sheet.

**Primary Dependencies**:

- No new third-party crates. lopdf 0.34 already provides `is_encrypted()`
  and the writer the fixture generator uses. sha2 is already in the server's
  tree.
- The new path crates are `thunderforge-sheet-import` and
  `thunderforge-system-dnd5e-sheet`, plus `thunderforge-system-roll-for-shoes-sheet`
  for US4.
- The new wasm package is `@thunderforge/sheet-dnd5e`, built by
  `scripts/shared.mjs` beside `buildPdf()`.

**Storage**:

- PostgreSQL gets three migrations, numbered after
  `2026-10-07-120000-0000_roll_facets` (data-model.md).
- RustFS uses the existing bucket under the `sheets/` prefix.

**Testing**:

- `cargo test -p thunderforge-pdf`, covering the region, bounds and
  encryption tests;
- `cargo test -p thunderforge-sheet-import`, covering the plan invariants;
- `cargo test -p thunderforge-system-dnd5e-sheet`, against the generated
  fixtures;
- `cargo test -p thunderforge-system-dnd5e`, covering the validators and
  refine;
- `make test-rust ARGS="-p thunderforge-server"`, which needs the
  `thunderforge-canvas-assets` bucket;
- `pnpm -F @thunderforge/web test`;
- `pnpm e2e:sheet-import` and `pnpm e2e:sheet-import:standalone`;
- then every slice that `pnpm e2e:which --diff` names. The schema and the
  migrations are cross-cutting, so it prints FULL SUITE, and the named
  slices run instead. The full suite is never the gate.
- `pnpm playtest` for FR-061, last.

**Target Platform**: the server on Linux; the web in Chromium, as the e2e
suite runs.

**Project Type**: a pnpm and Cargo workspace (apps, crates, packs).

**Performance Goals**:

- A five-page export is read in the browser in under 2 s on a laptop.
- The server's preview and apply each finish in under 1 s, excluding upload
  time.
- SC-001 asks for the whole import in under two minutes, which is mostly
  the person reading the review.

**Constraints**:

- Shared code never names a system (`scripts/check-system-registry.mjs`).
- No Rust file goes over 1000 lines (`scripts/check-file-length.sh`). The
  large web files, `ActorDetailPage.tsx` (831 lines) and the 5e
  `ActorSheet.tsx` (1853), take only mount points. New sections go in new
  files.
- No client-side database.
- No committed personal sheet. Fixtures are generated and deterministic.
- Telemetry is anonymous and carries no ids or content (Principle VII).
  Tests run with `TELEMETRY=false`.

**Scale/Scope**: five stories, about 60 functional requirements, 3
migrations, 10 new e2e specs, a standalone reader suite and 1 playtest scenario.

## Constitution Check

| Principle | How this plan holds it |
| --- | --- |
| I. The server is the authority | The server re-reads the uploaded file natively, plans again, and writes only on a matching plan hash. Permission, the flag, the bounds, origin and the staged state are all checked on the server. A reading sent by the client is never written. |
| II. React and Bevy are isolated from the network | The review route and the staged queue call `apps/web/src/api/sheetImport.ts` through hooks that expose `refetch()`. The actor reaches the world store through event 39's sync, like any actor change. Bevy is not touched: staged content has no ability id, so it never reaches the board. |
| III. Optimistic updates with rollback | An import is deliberately not optimistic. The review is the preview, and the actor changes when the server confirms. A GM's decision on staged content is not optimistic either, because a refused decision must not show as made. |
| IV. Base data vs derived data | Skill and save modifiers, passives, initiative and the proficiency bonus are cross-checked and never stored (R6). `level` is kept as the sum of `classes`, which is validated, not trusted. |
| V. One pub/sub backplane | Events 39-41 go through `record_world_event` inside each change's transaction. There is no new channel. |
| VI. Every feature is proven by its own slice | The new `sheet-import` slice owns `sheet-import-*.spec.ts` and has a standalone half. Its neighbours (actors, compendium, combat, book-import, collections, accounts) are declared. `pnpm e2e:sheet-import`, plus every slice `pnpm e2e:which --diff` names, proves each story. The full suite is never the gate. |
| VII. Telemetry is on and anonymous | Five server instruments and one browser event are declared against spec 086's contract (R16). The only attributes are closed-set outcomes, certainties and pack ids. There are no names, ids or file content. Until 086 lands, the same points are `tracing` spans. |

Further gates:

- **III, audit**: each new table carries `created_by` and `updated_by`.
- **IV, ADR**: ADR-115, "Character sheets are read twice by one reader",
  lands with the change.
- **Layout**: the new crates go in `crates/` and `packs/systems/<id>/`.

## Project Structure

### Documentation (this feature)

```text
specs/048-bringing-a-character-in/
├── spec.md
├── plan.md              # this file
├── research.md          # R1–R19
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── graphql-sheet-import.md
│   ├── sheet-import-core.md
│   └── sheet-mapping-5e.md
└── tasks.md
```

### Source code (touched)

```text
crates/
├── thunderforge-pdf/src/        lib.rs (Limits, errors) · region.rs (new) · region_tests.rs (new) · content.rs (Do, if measured) · wasm.rs (read_page_text)
├── thunderforge-sheet-import/   NEW: character.rs · reader.rs · mapping.rs · plan.rs · hash.rs · tests/
├── thunderforge-canvas-core/src/system_contribution.rs   SheetImport slot
└── thunderforge-server/
    ├── migrations/2026-10-09-1{0,1,2}0000-0000_*          three migrations
    └── src/
        ├── sheet_import/        NEW: mod.rs · preview.rs · apply.rs · rollback.rs · storage.rs · snapshot.rs · route.rs
        ├── staged_content/      NEW: mod.rs · decide.rs · guard.rs · report.rs
        ├── graphql/             mutations_sheet_import.rs · mutations_staged_content.rs · queries/sheet_import.rs (+ _tests.rs each)
        ├── graphql/             rollCheck / makeAttack / ability-use / share call staged_content::guard
        ├── auth/world_membership.rs                      require_manages_content
        ├── compendium/origin.rs                          read the column
        ├── storage/rustfs.rs                             delete_object allows sheets/
        ├── settings/registry/declarations.rs · settings/features.rs   feature.sheet_import
        ├── world_events.rs                               codes 39–41
        └── users/mod.rs · users/export_content.rs        export v4 and deletion
packs/systems/
├── dnd5e/
│   ├── system.json              sheetImport · damageTypes · vocabulary · new fields
│   ├── server/src/              sheet_import.rs (refine) · validators.rs · lib.rs (slot) · models.rs (deleted)
│   ├── sheet/                   NEW crate: lib.rs · recognise.rs · fields.rs · glyphs.rs · content.rs · wasm.rs · tests/fixtures/gen.rs · examples/measure_corpus.rs
│   └── web/src/                 sheet/{Classes,Defences,Persona,Coins}Section.tsx · sheet/LinkedContent.tsx · index.ts (sheetReader)
└── roll_for_shoes/              system.json sheetImport · sheet/ (NEW crate) · server/src/lib.rs (slot) · web/src/index.ts
apps/web/
├── src/api/sheetImport.ts · src/api/graphqlClient.ts (x-tf-last-event)
├── src/pages/world/actor/systemSheetReaders.ts        NEW, globbed like systemActorSheets.ts
├── src/pages/world/actor/ActorDetailPage.tsx          the button (mount only)
├── src/pages/world/actor/import/                      NEW: SheetImportPage.tsx · FieldRow.tsx · ContentRow.tsx · ImportHistory.tsx
├── src/pages/world/players/PlayersPage.tsx            the row entry, after the hotfix
├── src/components/world/staged/                       NEW: BroughtByPlayers.tsx · StagedRow.tsx
├── e2e/sheet-import-*.spec.ts                         NEW
└── playtest/bring-a-character.playtest.ts             NEW
scripts/shared.mjs · scripts/e2e/slices.json · Dockerfile (cook the sheet crates)
docs/guides/bringing-a-character-in.md (new) · docs/guides/characters-for-your-players.md (link)
docs/CONTRIBUTING.md · docs/adrs/20261008-115-character_sheets_are_read_twice_by_one_reader.md · docs/adrs/README.md · .env.example
```

**Slice**: the new `sheet-import` slice, added by T004, owns
`sheet-import-` and has a `:standalone` half. Its neighbours are:

- `actors`: `actor-detail-routes.spec.ts`, plus `players-hero-edit.spec.ts`
  after the hotfix;
- `compendium`: `abilities-compendium.spec.ts` and `world-compendium.spec.ts`;
- `combat`: `combat-attack.spec.ts` and `dnd5e-sheet.spec.ts`;
- `book-import`: `pdf-reader.spec.ts`;
- `collections`: `content-origin.spec.ts` and `library-account-deletion.spec.ts`;
- `accounts`: `user-data-export.spec.ts`.

## Open items for the owner

Everything else in this plan is decided. Each item below has a default, and
the work proceeds on the default unless the owner says otherwise.

1. **Whose kept file follows a rescued character when an account is
   deleted?** When a GM's account is deleted, a player's actor is copied to
   the player (`rescue.rs`).
   - **Default**: the file stays with the account that uploaded it, and is
     deleted with that account. The rescued actor keeps its import records,
     marked "file no longer kept".
2. **Which second system proves SC-006?**
   - **Default**: Roll for Shoes, the other full-time system, with a reader
     for a one-page sheet that ThunderForge defines: name, skills with their
     levels, and XP. Its committed fixtures are generated.
   - No publisher prints a Roll for Shoes sheet, so the layout is ours. Only
     the scope comes from the game.
3. **File bounds.**
   - **Default**: 10 MB and 20 pages. A D&D Beyond export is five pages and
     under 1 MB.
   - The bounds are constants in the reader call, not settings, so changing
     them is a one-line change.

## Complexity Tracking

| Addition | Why it is needed | Simpler alternative rejected because |
| --- | --- | --- |
| Reading twice, browser and server, with a plan hash | Review must be instant and write nothing. The kept file must be the file that was read (Q2=B, FR-001). | Upload-first keeps a file for every decline. Trusting the browser's reading breaks FR-001 and SC-003 (R1). |
| A new neutral crate between the PDF crate and the pack | FR-001a and FR-011 both hold only if "a character" lives somewhere that is neither PDF nor 5e. | `thunderforge-content` is a book's model, and canvas-core must not depend on a PDF parser (R3). |
| A staged-content table, not a flag on `world_abilities` | Unadopted content must be absent from play by construction (FR-037). | A flag needs a `WHERE` on every play query, and the one that is missed leaks a declined spell (R8). |
| An `origin` column with no default on three tables | FR-033d forbids a permissive Authored default. | `content_origin()` hard-coding Authored is exactly what FR-033d rules out (R9). |
| A request header for last-applied event | FR-038b: a stale honest client must not be reported. | Server-side delivery tracking per viewer is new state on every event fan-out, while the store already knows the id (R13). |
| A second system's reader in US4 | SC-006 can only be shown by a second system. | A test-only mock pack proves the trait compiles, not that a real pack needs nothing outside itself. |
