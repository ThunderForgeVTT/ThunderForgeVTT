---
description: "Task list for specs 049 and 050, importing a source book and the account's library"
---

# Tasks: Importing a Source Book, and the Account's Library

**Input**: Design documents from `/specs/049-importing-a-source-book/`

**Prerequisites**: plan.md, research.md, data-model.md,
contracts/content-patterns.md, contracts/import.md, quickstart.md

**Scope**: One arc over **two specs** — [049](./spec.md) and
[050](../050-the-account-library/spec.md). The owner chose the merge on
2026-09-12 (research §1). Story labels name their spec: `[049-US1]`,
`[050-US3]`.

**Proof**: end-to-end tests and two measurement runs, not unit tests alone
(049 FR-065). Every phase ends with a proof task naming what it establishes.
`pnpm verify` does **not** type-check the web app, so every web phase runs
`pnpm -F @thunderforge/web exec tsc --noEmit` of its own (Principle V).

**Two gates, both late**: Phase 11 is evidence before the delta model; Phase 15
needs an owner-signed ADR before it starts. Everything before each still ships.

---

## Phase 1: Setup

- [X] T001 Record the baseline: run `cargo run -p dnd5e-server --example harvest -- <corpus bestiaries>` and keep the report. **The 717/747 figure could not be reproduced and has been retired**: it was measured over an unrecorded subset. Re-running the bestiary directory alone gives 451/538; the whole 246-book corpus gives **2155 creatures, 1880 reaches, 153 uncertain, 0 books failing to open**. That is the baseline from here because it is the only set anybody can specify exactly and re-run — recorded with its caveats in `measurements.md`
- [X] T002 [P] Read `specs/049-importing-a-source-book/contracts/content-patterns.md` and `contracts/import.md` into the working set — they are what every phase below is measured against
- [X] T003 [P] Corpus reachable at the owner's own library, **246 PDFs exactly**, and the T001 harvest run opened all 246 with **0 failures**. The "196 with text" half was *not* re-measured — `survey` was not re-run, because T023 extends it in phase 4 and measuring twice with the old tool buys nothing

---

## Phase 2: Foundational — a system declares its content patterns

**Purpose**: The extension point every reader below reads. **Blocks all
phases.**

**Goal**: A system pack says what its content looks like, and shared code finds
it without learning a word of that system's vocabulary.

**Independent test**: `node scripts/check-system-registry.mjs` passes with no
new exemption, and a system declaring nothing is refused with a reason.

- [X] T004 `ContentPattern` / `ContentPatternField` in `crates/pack_system_spec/src/lib.rs`, beside `SystemVision`, as `pub content_patterns: Option<Vec<ContentPattern>>` on `SystemManifest` with `#[serde(default, rename_all = "camelCase")]`. Duplicated from the runtime type rather than imported, exactly as `SystemVision` is and for the reason its doc comment already gives
- [X] T005 [P] Validation in `validate_system_manifest` in the same file: reject two patterns sharing an `anchor`, reject a duplicate `kind`, and reject a `prose` pattern that declares `fields` — a silently ignored field list is a promise somebody thinks they have (contract: content-patterns, "rules a pack must satisfy")
- [X] T006 [P] `crates/thunderforge-canvas-core/src/content_patterns.rs` — the runtime declaration, mirroring `vision_declaration.rs`: `ContentPatterns`, `Pattern { kind, shape, anchor, name_rule, fields }`, `Shape::{Anchored, Prose}`, `NameRule`, `FieldSpec { key, label, kind }`, plus `Default` returning an empty set
- [X] T007 `src/server/src/content_patterns.rs` — the loader, mirroring `vision_profiles.rs::vision_declaration_for_system`: read `<systems_dir>/<system_id>/system.json`, deserialize the `contentPatterns` key only, and treat missing/unreadable/malformed as absent rather than as an error
- [X] T008 [P] The 5e declaration in `packs/systems/dnd5e/system.json`: five kinds, with the creature `name` rule expressing "walk back up to six lines and take the largest" — the test this design had to pass. **Two shapes were corrected against real books rather than taken from the spec**: a magic item is a heading name then an *unlabelled* `Weapon (whip), uncommon` line, and a feat is a heading name then a `Prerequisite:` line. Neither has an anchor label, so both are prose. Spec 049's sentence calling magic items anchored is wrong and ADR-096 records the correction (049 FR-013)
- [X] T009 [P] Twelve tests in `crates/pack_system_spec/src/content_patterns.rs` covering each validation rule, plus two against the shipped 5e manifest so schema and file cannot drift. The drift test **failed immediately for a pre-existing reason**: no shipped manifest passes `validate_system_manifest` at all (`author`/`packages` vs the schema's `authors`/`packs`), so the validator has never been run against a real pack. Narrowed to `validate_content_patterns`, with the reason in its doc comment, and the wider problem filed as its own task
- [X] T010 [P] `docs/adrs/20260912-096-content_patterns_as_a_manifest_extension_point.md`, PROPOSED: an extension point in the shape `vision` already established, why the schema crate duplicates rather than imports, and why the declaration lives in the manifest rather than in a pack's Rust (it has to be readable by the browser, which is where the reading happens)
- [X] T011 Proved: `cargo test -p pack_system_spec` **70 passed**, `cargo test -p thunderforge-server content_patterns` **6 passed**, `check-system-registry` passes with `KNOWN` still empty, and 11 of 12 `pnpm verify` checks green — the twelfth (`sdk bindings`) greps `git status` and can only pass once the generated TS is committed. `lib.rs` crossed the 1000-line ceiling and was split into `content_patterns.rs`, which is where the block belonged anyway. A system with no declaration yields an **empty** declaration here; turning that into a refusal with a reason is phase 5's (T032/T036), and the loader's doc comment says so

**Checkpoint**: the declaration exists and nothing reads it yet.

---

## Phase 3 (049-US5, 049-US1): The anchored reader

**Goal**: Labelled-field content — spells, items, creatures — read from a real
book through the declaration, by code that names no system.

**Independent test**: `harvest` still finds 717 creatures and 747 reaches,
through the generic reader rather than through `statblock.rs`'s own machinery.

- [X] T012 [049-US5] `src/server/src/content/anchored.rs` — takes `thunderforge_pdf::layout::Line`s and a `Pattern`, finds each anchor, reads the declared fields, and applies the `NameRule`. It contains no game vocabulary: every label it looks for arrives from the declaration
- [X] T013 [P] [049-US1] `ReadValue` as an **enum** — `Clear(String) | Uncertain(String) | Unread` — in `crates/thunderforge-canvas-core/src/content_entry.rs`, not the `{ state, value }` pair this task specified. The pair can express "unread, and here is the value anyway", and anything expressible eventually gets written; the enum makes FR-002 structural instead of a rule somebody has to keep. It lives in canvas-core rather than the server because a pack refinement's `fn` pointer has to name it — `read` / `uncertain` / `unread`, and **`unread` carries no value** (049 FR-002, data-model "read value"). A value that will not parse as its declared type is `uncertain` with the text as read, never coerced and never dropped
- [X] T014 [049-US5] `refine_content: Option<ContentRefineFn>` on `SystemContribution`, where `ContentRefineFn = fn(&mut Entry, &[SourceLine])`. It fills `Entry::extras`, an opaque `serde_json::Value` that shared code carries and never looks inside — which is what lets a pack own a reading only it understands. The entry types moved to `content_entry.rs` in canvas-core to make this expressible, deliberately **without** a dependency on the PDF crate: the guards that need it stayed in the server, because dragging a document parser into a crate the engine compiles to wasm would cost every player who never imports anything
- [X] T015 [049-US5] Per-attack reach moves to a 5e refinement in `packs/systems/dnd5e/server/src/`, submitted through the existing `inventory::submit!` in that pack's `lib.rs`. Spec 045's playtest established reach is **per attack**, not per creature size, so it cannot be dropped — and it is not a labelled field, so it cannot be declared (contract: content-patterns, "what the declaration deliberately cannot express")
- [X] T016 [049-US5] `statblock.rs`, `statblock_tests.rs` and the pack's own `examples/harvest.rs` deleted, and the pack's now-unused `thunderforge-pdf` dependency with them — the refinement works on shared types alone. Safe because `statblocks()` had **no production caller**
- [X] T017 [P] [049-US5] The measurement moved to `src/app/examples/harvest.rs` rather than being pointed at the new path in place: the pack cannot see the server's reader, and `src/app` is the only crate that links the declaration, the generic reader **and** the pack's refinement. It takes the system id and kind as arguments, never as literals
- [X] T018 [P] [049-US1] Unit tests for `anchored.rs` in `src/server/src/content/anchored_tests.rs` — a field present, a field absent (unread, no value), a field that will not parse (uncertain, text kept), two entries in sequence, and an anchor appearing inside another entry's prose
- [X] T019 [049-US5] **Proved, and it beat the reader it replaces.** Over the same 246 books: **2750 creatures and 3049 reaches**, against the baseline's 2155 and 1880 — +28% and +62%. The reader that knows no game system finds more than the one written for this one; parity was the hope. Recorded in `measurements.md` with the caveat that this is a floor on recall and says nothing about precision, and that uncertain rose with it (153 → 221).

      All 12 `pnpm verify` checks green; `check-system-registry` still passes with `KNOWN` empty; 1397 server tests pass. **8 fail, all pre-existing and environmental** — `exploration::tests` and `users::tests` need a database carrying spec 045's migration (`column scenes.exploration_enabled does not exist`), and nothing in this phase goes near either module

---

## Phase 4 (049-US1): The prose reader, and the corpus measurement

**Goal**: Class features and feats captured as name, text and provenance — with
no mechanical fields, because there are none to find.

**Independent test**: a real book's class features come out named and complete,
and no entry has acquired a damage value or a range.

- [X] T020 [049-US1] `src/server/src/content/prose.rs` — finds entries by the declared `name` rule (bold or heading) and runs each to the next one. No fields, and the type gives it nowhere to put one (049 FR-001b, data-model "entry")
- [X] T021 [P] [049-US1] Tests in `src/server/src/content/prose_tests.rs` including the boundary case that matters: a bold run-in name *inside* a paragraph must not start a new entry
- [X] T022 [049-US1] Propagate `thunderforge_pdf`'s per-line `suspect` flag onto any entry built from those lines (049 FR-004), so the review can show which entries came from pages the reader struggled with
- [X] T023 [049-US1] `src/app/examples/harvest.rs` gained prose kinds, so one tool measures both shapes. `survey.rs` was left alone: harvest already walks the corpus and reports per kind, and a second tool over the same books is the disagreement this task was trying to avoid
- [X] T024 [049-US1] **Not a hand count, and recorded as not being one.** A hand count means a person reading the book. What ran instead is an independent structural check: the reader anchors on `Casting Time`, so a *different* label in the same block estimates how many blocks exist. Against `Components` the reader is exact (10/10, 150/150); against `Duration` it is 91% and 89% — straddling SC-002's bar. **SC-002 stays outstanding**; calling it met on two disagreeing measures would be the confident wrongness this spec exists to prevent
- [X] T025 [049-US1] **Proved, and the measurement found a design gap rather than confirming a number.** Declared as "a name, then paragraphs", the prose reader returned **72,974 magic items** across 246 books — including `Table of Contents` — and the *identical* count for feats, because two prose kinds described that way are indistinguishable. A prose pattern now must declare `confirmedBy`, refused at install and again at runtime. After it: magicItem **2073**, feat **1674**, and uncertain fell 2935 → 63 and 71. 5e declares no `classFeature` at all (FR-013a): a bold run-in name is not line-level bold, and zero is honest where 42,295 wrong entries are not. Spec FR-013 amended, FR-013b added, ADR-096 extended, and the spec's stale opening figures corrected.

      Original wording of this task: `specs/049-importing-a-source-book/measurements.md` in the shape spec 043's took — corpus provenance, verdict rule, results, threats to validity, what would reopen it — with numbers in generated JSON carrying `generatedAt`/`generatedBy`. If SC-002 is not met, **correct the spec** rather than leaving it aspirational

**Checkpoint**: a book can be read. Nothing has been sent anywhere.

---

## Phase 5 (049-US1): The confirmation window

**Goal**: Everything found, shown before anything leaves the machine, with an
explicit submit.

**Independent test**: an e2e test that watches the network fails if any request
body carries entry text before submit is pressed.

- [X] T026 [049-US1] `apps/web/src/services/bookImport.ts`, reading a chunk of pages at a time and yielding between chunks. Getting the readers there needed the refactor first: they moved to `crates/thunderforge-content`, which depends on neither the PDF layer nor the server so it compiles to wasm and to native, and `thunderforge-pdf` gained a `read_content` export. **The entries are built in the browser, not the lines shipped somewhere to be interpreted** — sending the lines would be sending the book
- [X] T027 [P] [049-US1] Hash the file with WebCrypto and ask the server whether **this account** already has it, before any content is read (049 FR-047, spec 047 FR-072). A hash is not content
- [X] T028 [049-US1] `apps/web/src/components/import/` — `BookReview.tsx` (grouped, counted, entries openable to fields and page, three read-states rendered three ways with `data-read-state` on each field), `ImportReview.tsx` (the window: hash, read with progress, hand back what was approved) and `selection.ts`, which holds the Game Master's decisions **beside** the read rather than in it. `approved()` is the single crossing from "what the review showed" to "what is submitted", which is what makes FR-027 testable at all
- [X] T029 [P] [049-US1] Exclusion in the review: whole kinds, and individual entries (049 FR-026)
- [X] T030 [P] [049-US1] Correction of an uncertain value in the review, carried into what is submitted (spec 048 FR-022, referenced by 049 FR-023)
- [X] T031 [049-US1] Silent pages reported, and a book that yielded nothing renders a refusal with **no submit button at all** rather than a disabled one — there is nothing to submit, and a greyed-out button invites a person to wonder what they did wrong (049 FR-005)
- [X] T032 [049-US1] **Done with the library page, and in a different form than written.** 049 FR-028's guard belongs with the route into the importer, and under the merged arc a book is read into the *account's* library — whose page does not exist until phase 13. The review component offers no route to itself, so nothing is currently reachable by anyone; so it was built with the account library page instead. FR-028's literal wording — "a Game Master, from that world's panel" — could not be implemented, because an account-level surface has no world and so no Game Master role to check. What was built is the guard that means something there: no query accepts an account id, `require_account_owner` has no administrator bypass, and a missing book answers identically to somebody else's. Proved twice — a Rust test where a stranger *and an operator* get an empty shelf, and a browser test where a second account holding the owner's book URL sees "not yours" with none of the content on the page. **FR-028 amended to match**
- [X] T033 [049-US1] `apps/web/e2e/book-import-review.spec.ts`, **3 passed in Chromium**. One test proves the declaration-driven readers run as wasm with no server involved and that an unfound field carries no value at all; one builds a two-page book whose second page has no text and proves the silent page is counted (`pages` 2, `silentPages` 1) while the readable page still yields its creature; and the third watches every request body and fails if any carries the book's text. Absence is not provable by inspection — a telemetry call added next year would pass a code review and break FR-020
- [X] T034 [049-US1] Proved, and verified independently of the report claiming it: **8 e2e passing in Chromium** (exclusion by entry and by kind, close leaves `submitted === null`, an all-images book refused rather than imported empty) and **13 vitest** on the rendering and the decision model. `tsc --noEmit` clean, `pnpm verify` 12/12

---

## Phase 6 (049-US2): Sending

**Goal**: A large book goes up visibly and applies completely or not at all.

**Independent test**: an induced failure partway leaves the account holding
nothing from that book.

- [X] T035 [049-US2] `createCompendiumFromImport` in `src/server/src/graphql/mutations_compendium.rs`, with the input from `contracts/import.md`. **Origin is not an input** — the server writes it (049 FR-051)
- [X] T036 [049-US2] Server re-checks on arrival, all four, before anything is written: the caller is the owning account, the system exists and declares patterns (re-read from the manifest, not trusted from the payload), the entry count is within bound, and every entry's kind is one the system declared. A review in a browser is not a permission (049 FR-036, Principle III)
- [X] T037 [049-US2] Apply inside `conn.transaction(...)`, following `src/server/src/map_import/mod.rs:230` — one row at a time inside one transaction, which is what every existing bulk write in this repository does
- [X] T038 [P] [049-US2] The stated entry bound (049 FR-035), refused in the browser before sending **and** on the server. It is what keeps the transaction sane, so it belongs in the contract rather than being discovered at the database
- [X] T039 [049-US2] `apps/web/src/api/compendium.ts` — submit with progress that reflects bytes actually sent and says *what* is being sent (049 FR-030, FR-031). Honest about what the bar means: reaching the end is "the server has it and is applying it", not "applied"
- [X] T040 [P] [049-US2] Abandonment (049 FR-034): dropping the request leaves nothing, because nothing partial was ever committed
- [X] T041 [049-US2] `apps/web/e2e/book-import-commit.spec.ts` — induce a failure partway and assert the account is byte-identical to before (049 SC-007)
- [X] T042 [049-US2] Proved: that e2e green; `cargo test -p thunderforge-server` green; `tsc --noEmit` clean

---

## Phase 7 (049-US3, 050-US1): The compendium and the account's library

**Goal**: What was imported lives on the account's shelf, browsable and
removable.

**Independent test**: two books, two compendiums; a re-import of one is caught
**from a different world**, which is the whole point of hashing against the
account rather than a world.

- [X] T043 [050-US1] Migration in `src/server/migrations/2026-09-12-100000-0000_compendium/`. **FR-057 and FR-001b are enforced by the schema, not by code discipline**: `origin` is a Postgres enum with no default and a BEFORE UPDATE trigger that refuses a change (verified by hand against the live database — the flip is refused citing the FR, a rename still succeeds), and a CHECK constraint refuses a prose entry carrying mechanical fields. `UNIQUE (owner_user_id, source_hash)` is per account, deliberately not global — `compendiums` and `compendium_entries`, **owned by an account** (049 FR-040), with `created_by`/`updated_by` per the convention `world_collections` sets. Never world-owned "for now"; research §7
- [X] T044 [050-US1] `src/server/src/auth/account_ownership.rs` — `require_account_owner`, fail-closed, beside `require_world_member`. It takes the **row** and looks the owner up itself, because handing it an owner id would be the inline comparison wearing a function name; and it has **no `is_admin` parameter at all**, since an account's library is not a shared space (FR-055a) and an argument that bypasses it should not exist. A missing row and a stranger's row refuse identically, so ids cannot be walked. **Net-new: no account-scope helper exists** and this arc asks the question in six places (research §11)
- [X] T045 [049-US3] `src/server/src/compendium/` — the store, with origin written on insert and **no update path to it at all** (049 FR-057). A value that can be flipped is a value that will be flipped
- [X] T046 [P] [049-US3] `src/server/src/graphql/queries/compendium.rs` — `myLibrary`, `compendium`, `compendiumEntries`, every one account-scoped through T044's helper, and none taking a `worldId`. **Known limit**: `compendiumEntries` paginates in the resolver rather than in SQL — the wire is bounded, the database read is not. Fine for books of the sizes measured; wants a keyset query in the store before anybody imports one with 10,000 entries
- [X] T047 [049-US3] `removeCompendium(id, confirm)` — two calls, because naming what is in use *before* confirming cannot be done in one. **FR-045 and FR-046 cannot yet be met in substance, and the emptiness is a fact rather than a stub**: nothing in today's schema can reference a `compendium_entries` row, because a world's book list is spec 050's; and a hand edit is a world's *delta over* the base rather than a change to the base, so there is nothing yet that could have been edited. The shape, the plumbing and a single extension point (`usage_of` / `hand_edited_in`) exist for spec 050 to fill
- [X] T048 [049-US3] Re-import against the account's library (049 FR-047, spec 047 FR-070–FR-075): recognised by hash, overwrite offered, hand-edited entries untouched, and a *near*-identical file **not** claimed as a match
- [X] T049 [P] [049-US3] `apps/web/src/pages/library/` — the account shelf: each compendium with its book, system, date, counts and origin. A **new account-level surface**, not a change to the world Compendium portal
- [X] T050 [049-US3] Proved: two books import as two compendiums with their own counts; a re-import is caught from a second world; removal names what is in use first; `cargo test -p thunderforge-server` and `tsc --noEmit` green

---

## Phase 8 (049-US4): Origin, and what may not leave

**Goal**: Uploaded content cannot leave the account, by any route, and the
refusal says why.

**Independent test**: every route out is refused for uploaded content and
allowed for authored content.

- [ ] T051 [049-US4] `docs/adrs/20260912-097-origin_as_a_non_editable_invariant.md` — **owner-signed, not implementer-signed**, on the precedent of ADR-069 and ADR-079 that liability decisions are the owner's. This phase does not start against an unsigned ADR
- [ ] T052 [049-US4] Enforce the invariant at the data boundary in `src/server/src/collections/`: a collection **cannot contain uploaded content by construction** (049 FR-054a), not by a check on each route. The routes known today are today's list
- [ ] T053 [P] [049-US4] Every refusal names the origin as the reason, and where the only remaining routes are authoring it or proposing it as a system pack, says so (049 FR-053, FR-056a)
- [ ] T054 [P] [049-US4] The licence of the uploaded document makes no difference (049 FR-056). An openly licensed PDF still yields unshareable content — the known cost, recorded in 049 decision 4
- [ ] T055 [049-US4] `apps/web/e2e/content-origin.spec.ts` — share, publish, export and collection-adoption each refused for uploaded content with a reason, and **each allowed for content authored by hand**. The rule restricts an origin, not a subject
- [ ] T056 [049-US4] Proved: that e2e green; ADR-097 signed and its status ACCEPTED; `pnpm verify` green

**Checkpoint**: 049 is complete. A book can be read, reviewed, sent, browsed,
and cannot leave.

---

## Phase 9 (050-US3, 049-US3): The book list

**Goal**: A world switches compendiums on, content is fetched rather than
copied, and players can see the list.

**Independent test**: one book in two worlds stores once; switching it off
leaves no copy behind.

- [X] T057 [050-US3] Migration for the book list in `src/server/migrations/` — a world's link to a compendium in **its owner's** library, naming the base version in force (050 FR-015). World-owned; nothing is copied
- [X] T058 [050-US3] `src/server/src/library/` plus `graphql/mutations_library.rs` — switch on, switch off. Switching on **fetches**; there is no copy step and no seeding step, and ticking at world creation is the same action as ticking later (050 FR-031, FR-033)
- [X] T059 [050-US3] A world may only switch on a compendium **its owner's account holds** (050 FR-014) — through T044's helper. A co-Game Master uses; they do not inherit
- [X] T060 [P] [050-US3] System matching: only compendiums read as the world's system are offered, and a world that changes system reports what no longer matches rather than continuing to serve it (050 FR-041, FR-042)
- [X] T061 [050-US3] Switching off names what is in use first, then removes the content with no copy left behind (050 FR-013, FR-032). Because content is fetched, this reaches a live table — so it is paid visibly, never discovered by a player whose sword vanished
- [X] T062 [P] [050-US3] The book list UI, and **players see it read-only** (050 FR-035, FR-036) — book names, nothing that amounts to content
- [X] T063 [049-US3] Browse by compendium in `apps/web/src/pages/world/compendium/`, beside the existing tabs — **049 FR-042, satisfied as written**, which is what merging the two specs bought
- [X] T064 [050-US1] Storage measurement into `measurements.md`: one book switched on in N worlds stores once (050 SC-001, SC-002, FR-071), generated not transcribed
- [X] T065 [050-US3] `apps/web/e2e/library-book-list.spec.ts` — stored content does not grow when a second world switches the same book on; switching off leaves nothing; a player sees the list and cannot change it
- [X] T066 [050-US3] Proved: that e2e green, the storage measurement recorded, `tsc --noEmit` clean

---

> **Phase 9 notes (verified 2026-09-13, commit `2481401`).** 1450 server tests and both book-list e2e pass on re-run, and the `world_books` trigger refusing a link to anybody else's shelf was confirmed against the live schema. Three places the spec did not survive contact:
> - **FR-015's "base version in force" has no version to name.** A compendium carries no version counter, so the link records the file hash plus the reader version — exactly what a re-import changes. Phases 11–12 may want a real counter.
> - **Switching off reports the book's whole contribution, not per-entry use.** Nothing in a world can reference an entry yet and there is no delta table, so `deltas` is honestly empty until phase 11.
> - **Only the world's *owner* may change its book list, not any Game Master.** The list draws on the owner's shelf, so a co-Game Master uses it and cannot change it. **This is an open decision for the owner**, recorded in `apps/web/PRODUCT.md`.
>
> Storage, measured on a synthetic 1,500-entry book (row bytes only, stated in `measurements.md`): one book in eight worlds stores 745,384 bytes against 5,953,856 if copied — **8.0×** — and each world's link is 144 bytes.

---

## Phase 10: The entry-identity measurement — **a gate**

**Purpose**: Decide by evidence how a delta re-attaches after a re-import,
**before** the delta model is built. Blocks Phase 11.

- [X] T067 `src/app/examples/identity.rs` rather than a survey mode — it needs the declaration, the readers and the pack's contribution, and `src/app` is the only crate that links all three. **The experiment as specified could not be run**: the corpus has no book in more than one file (246 books, no repeated hash, no repeated title), so the re-parse measured instead is the chunked-versus-whole read that actually ships in `bookImport.ts`
- [X] T068 Reported: **2740 of 2750 identities stable** across a genuine re-parse, **107 (3.9%) collide** within their own book, and **renames not measured** — that needs two editions of one work and the corpus has none, so it is left unanswered rather than approximated
- [X] T069 **Decided: FR-025 stands, amended.** Stability is excellent where it matters — every *real* entry survived, and a hypothesis that the ten differences would fall on chunk seams was tested and proved wrong: they sit mid-chunk and are all misparses (`hit: 14 (2d8 + 5) bludgeoning damage.` read as a creature name). A false positive depends on incidental context and so moves; a real creature does not.

      But 3.9% ambiguity is not zero, so **FR-025a is new**: where two entries share a kind and a name, a delta MUST NOT attach to either and the ambiguity is reported. A rule that cannot always identify an entry must refuse rather than guess, because guessing wrong rewrites the wrong entry and says nothing
- [X] T070 Proved: the measurement is in `measurements.md` with its method, its numbers, the hypothesis it falsified, and the question it could not answer. FR-025 stands with evidence and FR-025a is added. **Phase 11 is unblocked**

---

## Phase 11 (050-US2): Deltas

**Goal**: A world changes what it inherited without changing the base or any
other world.

**Independent test**: an edit in one world reaches no other world and no base.

- [ ] T071 [050-US2] Migration for deltas in `src/server/migrations/` — world-owned, three forms: entry **changed**, **hidden**, **added**, storing only what differs rather than a whole copy (050 FR-021, FR-023)
- [ ] T072 [050-US2] Resolution in `src/server/src/library/` — what a world reads is the base with its delta applied (050 FR-022), keyed by the identity Phase 10 settled
- [ ] T073 [050-US2] **Origin per entry, not per compendium** (050 FR-052, FR-052a): a change to an uploaded entry is uploaded and cannot be shared; an addition beside it is authored and can be. Same screen, same feature, different rights — the subtlety most likely to be implemented wrong
- [ ] T074 [P] [050-US2] Restore: a Game Master can see what an entry was before their world changed it, and put it back (050 FR-024)
- [ ] T075 [P] [050-US2] Removing a world removes its deltas and touches no base and no other world (050 FR-028)
- [ ] T076 [050-US2] Measure resolution cost and **fix 050 SC-004 / FR-072's margin from the measurement**, which the spec deliberately left as a number to be measured rather than guessed
- [ ] T077 [050-US2] `apps/web/e2e/library-deltas.spec.ts` — edit in one world, other world unchanged, **base byte-identical to what the import produced**; addition absent elsewhere; hidden entry present elsewhere; and the origin split provable on the changed-versus-added pair
- [ ] T078 [050-US2] Proved: that e2e green, the latency margin recorded in `measurements.md`, `cargo test` and `tsc --noEmit` green

---

## Phase 12 (050-US5): Re-import under deltas

**Goal**: A corrected book replaces the base and a world's changes survive.

**Independent test**: deltas still apply after a re-import, and any that cannot
re-attach are reported by name.

- [ ] T079 [050-US5] Re-import replaces the base as a new version, leaving deltas attached (050 FR-026). The base is immutable; a new reading is a new version, never an edit (050 FR-006)
- [ ] T080 [050-US5] A delta that can no longer attach is **reported by name and not silently discarded** (050 FR-027) — including the rename case, which Phase 10's rule makes a removal plus an addition
- [ ] T081 [P] [050-US5] A re-import that fails partway leaves the previous base in place and no delta touched
- [ ] T082 [050-US5] Proved: deltas in two worlds survive a re-import; an unattachable delta is named; a failed re-import changes nothing; `cargo test` green

---

## Phase 13: Collections on the same shelf

**Goal**: Authored content sits beside imported content and can be taken away.

**Independent test**: a collection downloads as JSON; an imported compendium
does not.

- [ ] T083 Collections on the library shelf (050 FR-007 to FR-009), behaving identically to a compendium everywhere origin does not decide the answer — shelf, book list, world, delta
- [ ] T084 [P] A collection carries a system and is offered only to worlds on it (050 FR-009)
- [ ] T085 Download a collection as JSON (050 FR-009a). It is theirs and they made it
- [ ] T086 [P] An imported compendium does not download, by that route or any other (050 FR-009b) — the existing rule, not a new one
- [ ] T087 A download containing anything derived from an import **excludes it and says so** (050 FR-009c). A silently thinner file is worse than a refused one
- [ ] T088 Proved: a collection downloads and opens outside ThunderForge; a compendium is refused; `pnpm verify` green

---

## Phase 14 (050-US4): The shelf goes when the account goes

**Goal**: Deleting an account deletes its whole library, with nothing retained.

**Independent test**: after deletion, nothing of that account's library remains,
verified by inspection rather than by assertion.

- [ ] T089 [050-US4] Account deletion deletes the entire library including bases **no world referenced** (050 FR-062, FR-064). There is no other account's copy to keep it for — which is what rejecting cross-account dedup bought (049 decision 1)
- [ ] T090 [050-US4] The existing rule still runs first: players' actors are copied to their own accounts, and **inherited content is not copied with them** (050 FR-063) — that would move a book between accounts
- [ ] T091 [050-US4] The content agreement on every import states what is true under this architecture (050 FR-054): for that person and their games, never shared between users, and deleting it deletes it — **with no shared copy retained behind the scenes, because there is none**
- [ ] T092 [050-US4] `apps/web/e2e/` — delete an account and verify by inspection that zero bytes of its library remain (050 SC-007, FR-083)
- [ ] T093 [050-US4] Proved: that e2e green; `cargo test -p thunderforge-server` green

---

## Phase 15 (050-US6): Sync-back — **gated**

**Goal**: A world's improvement reaches the shelf, for authored content only.

**⚠️ Do not begin this phase until both constitutional conditions are met.**

- [ ] T094 [050-US6] **(a)** Re-confirm the notice-and-takedown programme is operational — it was confirmed for ADR-069 and extended by ADR-079. Re-confirming, not re-establishing
- [ ] T095 [050-US6] **(b)** `docs/adrs/20260912-098-an_update_path_to_a_collection.md`, **amending ADR-069** and **signed by the accountable owner**. ADR-069's own limits name versioned collections and update paths to already-copied content as re-opening it, and 050 FR-104 is such a path. Research §9 states the question it must answer: spec 026 says an adopted copy is independent, so a sync-back plausibly does not reach copies others took — a reasonable reading, and not the implementer's to assert
- [ ] T096 [050-US6] Sync a world's change back to the **collection** it came from (050 FR-100), creating a new version of its base and leaving the previous one recoverable (050 FR-104)
- [ ] T097 [050-US6] **No route at all** from a world to an imported compendium's base, at any volume of change (050 FR-101), and the reason stated where the absence would be noticed (050 FR-102)
- [ ] T098 [P] [050-US6] Syncing back shows what will change on the shelf and requires explicit confirmation (050 FR-103) — it writes to something every other world is reading
- [ ] T099 [050-US6] A world that has synced no longer holds the change as a delta (050 FR-105); other worlds' deltas behave exactly as after a re-import
- [ ] T100 [050-US6] Extend `apps/web/e2e/library-deltas.spec.ts` — a synced change reaches a world started afterwards; the syncing world no longer holds it; and **no route exists** from a world to an imported base, attempted through every surface that offers the sync for collections (050 FR-089, FR-089a)
- [ ] T101 [050-US6] Proved: that e2e green; ADR-098 ACCEPTED and owner-signed; `pnpm verify` green

---

## Phase 16: Polish & cross-cutting

- [ ] T102 [P] Update `specs/049-importing-a-source-book/spec.md` and `specs/050-the-account-library/spec.md` status to what shipped, and correct any Success Criterion the measurements contradicted
- [ ] T103 [P] `docs/adrs/README.md` gains ADR-096, ADR-097 and ADR-098 — five accepted ADRs were once found missing from that index, so it is checked rather than assumed
- [ ] T104 [P] Run `node scripts/e2e-parallel.mjs` in full and record the suite's time and result beside the 29.7 minutes of 2026-09-11
- [ ] T105 Run `pnpm quickstart` checks from `quickstart.md` end to end, on a **real book**, not a fixture
- [ ] T106 Run `pnpm verify` and `pnpm -F @thunderforge/web exec tsc --noEmit`, and fix what they report **in the code this arc added**. A repo-wide lint remediation folded in here would bury the feature work; wide passes get their own commit

---

## Dependencies & Execution Order

### Phase dependencies

- **Phase 1 (Setup)**: no dependencies
- **Phase 2 (Foundational)**: blocks every phase below — nothing reads content without the declaration
- **Phases 3-4**: the readers. Depend on Phase 2
- **Phases 5-6**: review and send. Depend on the readers
- **Phase 7**: the store. Depends on Phase 6
- **Phase 8**: origin. Depends on Phase 7; **additionally gated on ADR-097**
- **Phase 9**: the book list. Depends on Phase 7 (not on Phase 8)
- **Phase 10**: the identity gate. Depends on Phase 4's reader; **blocks Phase 11**
- **Phases 11-12**: deltas, then re-import under them
- **Phase 13**: collections. Depends on Phase 9's book list
- **Phase 14**: account deletion. Depends on Phase 7; sensibly after Phase 13 so the shelf is whole
- **Phase 15**: sync-back. Depends on Phases 11 and 13; **gated on ADR-098**

### Story dependencies

- **049-US5** (a second system's book) is Phase 2-3 and comes first, because the declaration is what everything else reads
- **049-US1** (see it, then decide) spans Phases 3-5 and is the MVP
- **049-US2, 049-US3, 049-US4** follow in Phases 6, 7 and 8
- **050-US1, 050-US3** land in Phases 7 and 9
- **050-US2, 050-US5** need the Phase 10 gate first
- **050-US4, 050-US6** are last

### Parallel opportunities

- T002, T003 (setup)
- T005, T006, T008, T009, T010 — schema validation, runtime type, the 5e block, tests and the ADR are five files
- T013, T018 alongside T012
- T029, T030 — exclusion and correction are separate review controls
- T046, T049 — the query surface and the shelf UI
- T053, T054 — refusal wording and the licence rule
- T060, T062 — system matching and the player-visible list
- T074, T075 — restore and world removal
- T084, T086 — a collection's system and a compendium's refusal to download

---

## Implementation Strategy

### MVP

**Phases 1-5.** A Game Master picks a book, and sees everything that was found,
grouped and counted, with uncertainty visible — and nothing has been sent. That
is 049-US1, the spec's P1, and it is the risky half: reading a real book
correctly and refusing to invent. It is demonstrable on its own even though
nothing is stored yet.

### Incremental delivery

1. Phases 1-5 → **read and review** (MVP)
2. Phase 6-7 → **it is stored**, on the account's shelf
3. Phase 8 → **and it cannot leave** *(needs ADR-097)*
4. Phase 9 → **worlds can use it** — 049 complete, 050 begun
5. Phases 10-12 → **each world makes it its own**
6. Phases 13-14 → **authored content beside it, and a clean deletion**
7. Phase 15 → **improvements flow back** *(needs ADR-098)*

Each step is shippable. If either gate is refused or delayed, everything before
it still ships — which is why they are where they are (research §1, §9).

---

## Notes

- `[P]` = different files, no dependency on an incomplete task
- Story labels name their spec, because this arc covers two
- `pnpm verify` does **not** type-check the web app — run `tsc --noEmit` separately
- Two known process-global test flakes (engine `authoring_mode`, server
  `settings::resolver`): re-run with `--test-threads=1` before blaming a change
- E2E: `node scripts/e2e-parallel.mjs --shards=1 --only=<basename>` with
  `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1`
