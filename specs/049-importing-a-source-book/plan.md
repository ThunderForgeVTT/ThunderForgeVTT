# Implementation Plan: Importing a Source Book

**Branch**: `049-importing-a-source-book` | **Date**: 2026-09-12 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/049-importing-a-source-book/spec.md`

## Summary

A Game Master reads a rulebook off their own disk, sees everything that was
found before a byte of it leaves the machine, and gets it back as a compendium
on their account's shelf.

The reader already exists and is proven in a browser. What this plan builds is
everything between "the reader has finished" and "the account has a
compendium": a way for a game system to say what its content looks like, two
readers built on that declaration, a confirmation window that is the last word
before anything is sent, a send that applies completely or not at all, and a
store whose origin cannot be edited off.

The work is sequenced so each phase is provable on its own, in the pattern
spec 045 used. The proof here is not a playtest — it is end-to-end tests and a
measurement run against a real 246-book library, because every defect found
while building the reader produced text that looked plausible and was wrong.

| Phase | What lands | How it is proved |
|---|---|---|
| 1 | A system pack declares its **content patterns**; shared code reads the declaration and names no system | `check-system-registry` still passes with no exemption; the 5e pack declares five kinds |
| 2 | The **anchored reader** on those patterns, with the existing creature reader reached through it | The 717 creatures still read; anything the declaration cannot express is named, not preserved in silence |
| 3 | The **prose reader** — name, text, provenance, no invented mechanics | Class features and feats come out of a real book with no mechanical fields |
| 4 | The **confirmation window**: everything found, per-field certainty, explicit submit | An e2e test fails if any request carries book content before submit |
| 5 | **Sending**: real progress, all-or-nothing, abandonment, server re-checks on arrival | An induced mid-flight failure leaves the account unchanged |
| 6 | The **compendium**, account-owned, browsable, removable, hash-checked against the shelf | Two books, two compendiums; re-import offers overwrite |
| 7 | **Origin** recorded automatically and enforced as an invariant | No route puts uploaded content into a collection; authored content is unaffected |
| 8 | The **measurement run** over the real library | `measurements.md` plus generated JSON; SC-002 either met or corrected |

## Technical Context

**Language/Version**: Rust (server, native; `thunderforge-pdf`, native and
`wasm32-unknown-unknown`), TypeScript 5 with React 19 (web) — as the
repository already builds them.

**Primary Dependencies**: `crates/thunderforge-pdf` compiled to wasm and
shipped as `@thunderforge/pdf`; async-graphql + Diesel + Postgres (server);
Playwright (e2e). No new dependency in any layer.

**Storage**: Postgres, for compendiums and their entries, **owned by an
account** rather than a world (FR-040). The uploaded PDF is never stored — it
does not leave the Game Master's machine at all. Nothing is added to the client
world cache by this feature.

**Testing**: `cargo test` for `thunderforge-pdf` and the server; `vitest` for
web units; Playwright for the end-to-end proofs (FR-061 to FR-064); and a
corpus measurement run (FR-060) that is evidence rather than a test.

**Target Platform**: Chromium against a ThunderForge instance, per the
constitution's browser constraint. The reader is WASM in that browser.

**Project Type**: Existing multi-part repository — a shared crate, the server,
the web app and the system packs. No new service.

**Performance Goals**: A 350-page book reaches a reviewable list in under 30
seconds on a typical laptop (SC-004). Progress advances at least once every two
seconds while sending a 70 MB book (SC-006) — noting that what is sent is the
parsed content, not the PDF.

**Constraints**: Nothing read leaves the machine before an explicit submit
(FR-020, FR-024). Only a Game Master may read a book into their library, from
their own panel (FR-028). Shared code may name no game system (FR-012), which
`scripts/check-system-registry.mjs` already enforces. `pnpm verify` does **not**
type-check the web app — `pnpm -F @thunderforge/web exec tsc --noEmit` is a
separate per-phase check, as it was for spec 045.

**Scale/Scope**: Eight phases. Touches `thunderforge-pdf` (readers), the system
packs (a manifest block), the server (ingest, store, the origin invariant), and
the web (the import flow and the library view). A world cannot yet switch a
compendium on — that is spec 050, and research decision 1 records why the gap
is deliberate.

## Constitution Check

| Principle | How this plan satisfies it |
|---|---|
| **I. ECS owns simulation** | Not engaged. Nothing here draws on the canvas or reasons spatially; the importer is chrome around the engine, which is where the constitution says new UI belongs. No engine change in any phase. |
| **II. Plugin-modular engine** | Not engaged, for the same reason — no engine plugin is added or altered. |
| **III. Ownership at the data boundary** | The binding principle. The browser does the reading and the review, and the server re-checks authorization, the account's identity, the world's system and the entry bound **on arrival** — a review that happened in a browser is not a permission (FR-036). Origin is written by the server, never accepted from the client (FR-051). The new tables carry `created_by`/`updated_by` per existing convention. |
| **IV. ADRs before divergent implementation** | Two are required and both land with the change set, not after. **ADR-096, content patterns as a manifest extension point** — a new pack contract, the same kind of change spec 045 made to the `vision` block and spec 016 made before it. **ADR-097, origin as a non-editable invariant** — this one is about liability, so on the precedent of ADR-069 and ADR-079 it is **signed by the accountable owner, not the implementer**, and Phase 7 is gated on that signature. |
| **V. Verify before claiming done** | Per phase: `cargo check` (server and `thunderforge-pdf`), `cargo check --target wasm32-unknown-unknown` where the wasm build is touched, `pnpm -F @thunderforge/web exec tsc --noEmit` (web), then the phase's own e2e. |

**DMCA / Content Moderation Guardrail**: assessed and **not engaged by this
feature**. The guardrail fires on making one world's compendium content visible
or accessible outside that world; 049 creates content that is structurally
incapable of leaving the account that imported it, and FR-054a narrows what
ADR-069's determination has to carry rather than widening it. Research §7
records the finding, and records that **spec 050's sync-back may re-open
ADR-069** — that is 050's gate to clear before it is built, and is one of the
reasons this plan does not merge the two.

**Gate result**: PASS. No violations to justify. One phase (7) is gated on an
owner signature rather than on engineering, and the plan says so rather than
discovering it during implementation.

### Re-check after Phase 1 design

Re-evaluated against the generated `data-model.md` and `contracts/`. Still
PASS, with three things the design surfaced that the first pass had not:

- **Principle III got stronger, and needed to.** The design found that the
  server has `require_world_member` for world scope and **nothing at all** for
  account scope — account-owned rows are checked inline against
  `authenticated_user(ctx)`. This feature asks that question in six places, so
  it introduces the missing helper rather than inlining it a seventh time
  (research §11). That is the boundary acquiring a check it was missing, not
  new complexity.
- **Principle IV's first ADR is narrower than expected, and its second is
  unchanged.** Content patterns turn out to fit the `vision` precedent exactly
  — schema, runtime type, loader — so ADR-096 records an extension point in an
  established shape rather than a new mechanism. ADR-097 remains owner-signed.
- **One spec requirement cannot be met by this spec.** FR-042 asks for browsing
  by compendium *in the world's Compendium portal*; under research decision 1 a
  world cannot reach a compendium until spec 050. 049 delivers account-level
  browsing instead, and FR-042's world-portal half belongs with 050's book
  list. **This needs the owner's agreement before `/speckit-tasks`** — it is
  the one place the plan departs from the spec as written.

## Project Structure

### Documentation (this feature)

```text
specs/049-importing-a-source-book/
├── spec.md              # the feature specification, with its owner decisions
├── plan.md              # this file
├── research.md          # Phase 0: the decisions and why
├── data-model.md        # Phase 1: entities and state
├── contracts/
│   ├── content-patterns.md   # what a system pack declares
│   └── import.md             # GraphQL ingest, and what crosses the wire
├── quickstart.md        # Phase 1: how to prove each phase
├── measurements.md      # Phase 8: the corpus run (FR-060)
└── checklists/
    └── requirements.md  # written by /speckit-specify
```

### Source Code (repository root)

```text
crates/pack_system_spec/src/lib.rs   # phase 1: contentPatterns schema + validation
crates/thunderforge-canvas-core/src/
├── content_patterns.rs              # phase 1: the runtime declaration (mirrors vision_declaration.rs)
└── system_contribution.rs           # phase 2: a pack contributes what data cannot express
crates/thunderforge-pdf/
├── src/                             # unchanged: the reader is built
└── examples/survey.rs               # phase 8: extended to count entries per kind
packs/systems/dnd5e/
├── system.json                      # phase 1: the contentPatterns block
└── server/src/statblock.rs          # phase 2: superseded by the generic reader;
                                     #          per-attack reach stays, as a contribution
src/server/src/
├── content_patterns.rs              # phase 1: the loader (mirrors vision_profiles.rs)
├── content/anchored.rs              # phase 2: labelled-field reader, names no system
├── content/prose.rs                 # phase 3: name, text, provenance
├── compendium/                      # phase 6: account-owned store
├── auth/account_ownership.rs        # phase 6: the helper that does not exist yet
├── graphql/mutations_compendium.rs  # phase 5: ingest, one transaction
├── graphql/queries/compendium.rs    # phase 6: the library view
└── collections/                     # phase 7: the invariant lands at this boundary
src/server/migrations/               # phase 6: compendium + entries, account-owned
apps/web/src/
├── services/pdfReader.ts            # unchanged: the browser seam exists
├── services/bookImport.ts           # phase 2-4: patterns applied in the browser
├── components/import/               # phase 4: the confirmation window
├── api/compendium.ts                # phase 5: submit, with progress
└── pages/library/                   # phase 6: NEW account-level surface, not the world portal
apps/web/e2e/
├── book-import-review.spec.ts       # phase 4: nothing on the wire before submit
├── book-import-commit.spec.ts       # phase 5: all-or-nothing
└── content-origin.spec.ts           # phase 7: no route out for uploaded content
docs/adrs/
├── …-096-content_patterns_as_a_manifest_extension_point.md   # phase 1
└── …-097-origin_as_a_non_editable_invariant.md               # phase 7, owner-signed
```

**Structure Decision**: The repository's existing layout, and the `vision`
extension point's exact shape — schema in `pack_system_spec`, runtime type in
`thunderforge-canvas-core`, loader in `src/server/src` (research §3). This
feature adds one server module tree (`content/`, `compendium/`) and one new
account-level web surface; everything else extends a file that already owns
that concern. The parsing stays in `thunderforge-pdf`; the system-specific
vocabulary stays in the pack.

**Two things the map changes from a first reading of the spec.** The library
view is a **new account-level page**, not an edit to the existing world
Compendium portal at `apps/web/src/pages/world/compendium/` — FR-042's
world-portal integration moves to spec 050 with the book list it depends on
(research §1). And `src/server/src/auth/account_ownership.rs` is net-new: the
server has `require_world_member` and no account-scope equivalent at all
(research §11).

## Complexity Tracking

No constitution violations to justify. Three risks worth naming rather than
tracking as complexity:

| Risk | Why it is accepted | What contains it |
|---|---|---|
| **The readers run in two places** — the browser parses and reviews, the server re-checks. | A review in a browser is not a permission (Principle III), so the server cannot simply trust what arrives. | The *reader* is one crate compiled twice, never two implementations. What the server re-does is authorization and bounds, not parsing. |
| **One large transaction** on commit (research §4). | All-or-nothing is worth more than streaming, and a failure partway is the expensive case. | FR-035's stated entry bound, refused before sending, is what keeps the transaction sane. Staging-then-swap is the escape hatch if measurement says the bound is too small. |
| **Phase 7 waits on a signature**, not on code. | The origin invariant is a liability decision, and ADR-069/ADR-079 set the precedent that those are the owner's to sign. | Phases 1-6 do not depend on it; Phase 7 is the last of the build phases, so the wait is at the end rather than in the middle. |
