# Implementation Plan: Importing a Source Book, and the Account's Library

**Branch**: `049-importing-a-source-book` | **Date**: 2026-09-12
**Specs**: [049 spec.md](./spec.md) and [050 spec.md](../050-the-account-library/spec.md)

**Input**: Feature specifications from `/specs/049-importing-a-source-book/spec.md`
and `/specs/050-the-account-library/spec.md`

> **One arc, two specs.** The owner chose on 2026-09-12 to build 049 and 050
> together rather than in sequence (research §1). This is the plan for both;
> spec 050 has no separate plan. `tasks.md` here covers the whole arc.

## Summary

A Game Master reads a rulebook off their own disk, sees everything that was
found before a byte of it leaves the machine, gets it back as a compendium on
their account's shelf, and switches it on in whichever of their worlds want it
— where each world can change what it inherited without changing any other.

The reader already exists and is proven in a browser. Everything else is new:
a way for a game system to say what its content looks like, two readers built
on that declaration, a confirmation window that is the last word before
anything is sent, a store whose origin cannot be edited off, a per-world book
list that fetches rather than copies, and a delta model that lets one table's
changes stay that table's.

Sixteen phases in five arcs, each provable on its own. **Both gates sit in the
last third**, so the reader and the shelf ship regardless of how either
resolves.

### Arc A — reading a book (phases 2-4)

| Phase | What lands | How it is proved |
|---|---|---|
| 2 | A system pack declares its **content patterns**; shared code reads the declaration and names no system | `check-system-registry` still passes with no exemption; the 5e pack declares five kinds |
| 3 | The **anchored reader** on those patterns, with the existing creature reader reached through it | The 717 creatures and 747 reaches still read; what the declaration cannot express is named, not silently kept |
| 4 | The **prose reader**, and the **corpus measurement** | Class features come out with no mechanical fields; `measurements.md` says what the readers actually get out of 246 real books |

### Arc B — deciding, and sending (phases 5-6)

| Phase | What lands | How it is proved |
|---|---|---|
| 5 | The **confirmation window**: everything found, per-field certainty, explicit submit | An e2e test fails if any request carries book content before submit |
| 6 | **Sending**: real progress, all-or-nothing, abandonment, server re-checks on arrival | An induced mid-flight failure leaves the account unchanged |

### Arc C — the shelf, and worlds using it (phases 7-9)

| Phase | What lands | How it is proved |
|---|---|---|
| 7 | The **compendium and library**, account-owned, browsable, removable, hash-checked against the shelf | Two books, two compendiums; a re-import is caught from a *different world* |
| 8 | **Origin** enforced as an invariant *(needs ADR-097, owner-signed)* | No route puts uploaded content into a collection; authored content unaffected |
| 9 | The **book list**: a world switches compendiums on, content fetched not copied, players see it read-only | One book in two worlds stores once; switching off leaves no copy behind |

### Arc D — a world's own changes (phases 10-12)

| Phase | What lands | How it is proved |
|---|---|---|
| 10 | **The entry-identity measurement** — a gate, not a report | Corpus evidence either confirms kind-plus-name or replaces it, *before* phase 11 |
| 11 | **Deltas**: changed, hidden, added — with origin per entry | An edit in one world reaches no other world and no base |
| 12 | **Re-import** replaces a base with deltas over it | Deltas survive; any that cannot re-attach are reported by name |

### Arc E — authored content, lifecycle, and the last gate (phases 13-15)

| Phase | What lands | How it is proved |
|---|---|---|
| 13 | **Collections on the same shelf**, and JSON download for authored content | A collection downloads; an imported compendium does not |
| 14 | **Account deletion takes the library**, with nothing retained | Zero bytes remain, verified by inspection rather than assertion |
| 15 | **Sync-back** — a world's improvement reaches the shelf *(needs ADR-098, owner-signed)* | No route from a world to an *imported* base, at any volume |

Phase 1 is setup and phase 16 is polish. Phase 15 is last on purpose: if its
determination is refused or delayed, phases 1-14 still ship (research §1, §9).

**Two things task generation changed, recorded rather than left to drift.** The
corpus measurement moved from the end to **phase 4**, because it is what proves
the readers work on real books and finding that out last is finding it out too
late. And **account deletion gained a phase of its own** — spec 050 US4 and
FR-062 to FR-064 had requirements and an e2e but no phase, which is the kind of
gap that gets discovered when something is already built on top of it.

## Technical Context

**Language/Version**: Rust (server, native; `thunderforge-pdf`, native and
`wasm32-unknown-unknown`), TypeScript 5 with React 19 (web) — as the
repository already builds them.

**Primary Dependencies**: `crates/thunderforge-pdf` compiled to wasm and
shipped as `@thunderforge/pdf`; async-graphql + Diesel + Postgres (server);
Playwright (e2e). No new dependency in any layer.

**Storage**: Postgres. Compendiums and their entries are **owned by an account**
(049 FR-040); a world's book list and its deltas are world-owned and reference
them. The uploaded PDF is never stored — it does not leave the Game Master's
machine. Nothing is added to the client world cache by this arc.

**Testing**: `cargo test` for `thunderforge-pdf` and the server; `vitest` for
web units; Playwright for the end-to-end proofs; and two measurement runs —
the corpus read (049 FR-060) and entry identity (050 FR-029) — that are
evidence rather than tests.

**Target Platform**: Chromium against a ThunderForge instance, per the
constitution's browser constraint. The reader is WASM in that browser.

**Project Type**: Existing multi-part repository — shared crates, the server,
the web app and the system packs. No new service.

**Performance Goals**: A 350-page book reaches a reviewable list in under 30
seconds on a typical laptop (049 SC-004). Progress advances at least once every
two seconds while sending (049 SC-006). Resolving base-plus-delta is no slower
than reading world-owned content by a margin **fixed by measurement before
release** (050 SC-004, FR-072) — deliberately not a guessed number.

**Constraints**: Nothing read leaves the machine before an explicit submit (049
FR-020, FR-024). Only a Game Master may read a book into their library, from
their own panel (049 FR-028). Nothing is ever copied into a world (050 FR-031).
Shared code may name no game system (049 FR-012), which
`scripts/check-system-registry.mjs` already enforces. `pnpm verify` does **not**
type-check the web app — `pnpm -F @thunderforge/web exec tsc --noEmit` is a
separate per-phase check, as it was for spec 045.

**Scale/Scope**: Sixteen phases across two specs. Touches `pack_system_spec`,
`thunderforge-canvas-core`, `thunderforge-pdf`, the 5e pack, the server
(ingest, store, origin, book list, deltas) and the web (import flow, library,
book list). No engine change.

## Constitution Check

| Principle | How this plan satisfies it |
|---|---|
| **I. ECS owns simulation** | Not engaged. Nothing here draws or reasons spatially; this is chrome around the engine, which is where the constitution says new UI belongs. No engine change in any phase. |
| **II. Plugin-modular engine** | Not engaged, for the same reason — no engine plugin added or altered. |
| **III. Ownership at the data boundary** | The binding principle. The browser reads and reviews; the server re-checks authorization, the account, the system and the entry bound **on arrival** — a review in a browser is not a permission (049 FR-036). Origin is written by the server, never accepted from the client (FR-051). A world may only switch on a compendium **its owner's account holds** (050 FR-014). New tables carry `created_by`/`updated_by` per convention. |
| **IV. ADRs before divergent implementation** | Three, all landing with the change set. **ADR-096, content patterns as a manifest extension point** (Phase 2) — an extension point in the shape `vision` already established. **ADR-097, origin as a non-editable invariant** (Phase 8) — about liability, so **owner-signed** on the precedent of ADR-069 and ADR-079. **ADR-098, amending ADR-069 for an update path to a collection** (Phase 15) — see the guardrail below. |
| **V. Verify before claiming done** | Per phase: `cargo check` (server and the shared crates), `cargo check --target wasm32-unknown-unknown` where the wasm build is touched, `pnpm -F @thunderforge/web exec tsc --noEmit` (web), then the phase's own e2e. |

### DMCA / Content Moderation Guardrail — **engaged, in Phase 15 only**

Under the merge this arc does reach the guardrail, and it reaches it in exactly
one place. ADR-069's own limits say versioned collections and update paths to
already-copied content re-open its determination; spec 050 FR-104 is such an
update path.

Everything before Phase 15 is clear: uploaded content is structurally incapable
of leaving the account that imported it, and 049 FR-054a **narrows** what
ADR-069 has to carry rather than widening it.

Before Phase 15 begins, the constitution requires both:

1. **(a)** the notice-and-takedown program is operational — **re-confirmed as
   still true**, not re-established; it was confirmed for ADR-069 and extended
   by ADR-079;
2. **(b)** an explicit on-record determination, as **ADR-098 amending
   ADR-069**, signed by the accountable owner.

Research §9 states the question that determination must answer, so it is not
discovered late.

**Gate result**: PASS to begin. Phases 1-14 have no unmet gate. Phase 15 is
blocked until (a) and (b) above, and the phase order puts it last so that block
costs one feature rather than the arc.

## Project Structure

### Documentation (this feature)

```text
specs/049-importing-a-source-book/
├── spec.md              # the import specification
├── plan.md              # this file — covers 049 and 050
├── research.md          # Phase 0: the decisions and why
├── data-model.md        # Phase 1: entities and state
├── contracts/
│   ├── content-patterns.md   # what a system pack declares
│   └── import.md             # GraphQL ingest, and what crosses the wire
├── quickstart.md        # how to prove each phase
├── measurements.md      # Phase 13 output: corpus + identity + storage
└── checklists/requirements.md
specs/050-the-account-library/
├── spec.md              # the library specification
└── checklists/requirements.md   # no separate plan — see above
```

### Source Code (repository root)

```text
crates/pack_system_spec/src/lib.rs   # ph2: contentPatterns schema + validation
crates/thunderforge-canvas-core/src/
├── content_patterns.rs              # ph2: runtime declaration (mirrors vision_declaration.rs)
└── system_contribution.rs           # ph3: a pack contributes what data cannot express
crates/thunderforge-pdf/
├── src/                             # unchanged: the reader is built
└── examples/survey.rs               # ph4: entries per kind · ph10: identity across re-parses
packs/systems/dnd5e/
├── system.json                      # ph2: the contentPatterns block
└── server/src/statblock.rs          # ph3: superseded; per-attack reach stays as a contribution
src/server/src/
├── content_patterns.rs              # ph2: the loader (mirrors vision_profiles.rs)
├── content/{anchored,prose}.rs      # ph3, ph4: readers that name no system
├── compendium/                      # ph7: account-owned store
├── auth/account_ownership.rs        # ph7: the helper that does not exist yet
├── collections/                     # ph8: the invariant · ph13: collections on the shelf
├── library/                         # ph9: book list · ph11-12: deltas
├── graphql/mutations_compendium.rs  # ph6: ingest, one transaction
├── graphql/mutations_library.rs     # ph9: switch on · ph11: delta · ph15: sync back
└── graphql/queries/compendium.rs    # ph7, ph9: library and resolved reads
src/server/migrations/               # ph7: compendium + entries · ph9: book list · ph11: deltas
apps/web/src/
├── services/pdfReader.ts            # unchanged: the browser seam exists
├── services/bookImport.ts           # ph3-5: patterns applied in the browser
├── components/import/               # ph5: the confirmation window
├── api/compendium.ts                # ph6: submit, with progress
├── pages/library/                   # ph7, ph13: the account shelf
└── pages/world/compendium/          # ph9: browse by compendium (049 FR-042)
apps/web/e2e/
├── book-import-review.spec.ts       # ph5: nothing on the wire before submit
├── book-import-commit.spec.ts       # ph6: all-or-nothing
├── content-origin.spec.ts           # ph8: no route out for uploaded content
├── library-book-list.spec.ts        # ph9: stored once; switching off leaves nothing
└── library-deltas.spec.ts           # ph11: isolation · ph15: sync-back's asymmetry
docs/adrs/
├── …-096-content_patterns_as_a_manifest_extension_point.md   # ph2
├── …-097-origin_as_a_non_editable_invariant.md               # ph8, owner-signed
└── …-098-an_update_path_to_a_collection.md                   # ph15, owner-signed, amends ADR-069
```

**Structure Decision**: The repository's existing layout, and the `vision`
extension point's exact shape — schema in `pack_system_spec`, runtime type in
`thunderforge-canvas-core`, loader in `src/server/src` (research §3).

Two things worth flagging against a first reading. `apps/web/src/pages/library/`
is a **new account-level surface**; the existing world Compendium portal at
`apps/web/src/pages/world/compendium/` is extended in Phase 8 to browse by
compendium, which is 049 FR-042 satisfied as written — the merge is what makes
that possible. And `src/server/src/auth/account_ownership.rs` is net-new: the
server has `require_world_member` and no account-scope equivalent at all
(research §11).

## Complexity Tracking

No constitution violations to justify. Four risks named rather than tracked as
complexity:

| Risk | Why it is accepted | What contains it |
|---|---|---|
| **A large arc** — sixteen phases across two specs, the owner's choice over two sequenced releases. | It designs the compendium, the book list and the delta table together instead of migrating one to fit another. | Every phase is independently shippable and independently proved; the arc can stop after any of them. |
| **The readers run in two places** — the browser parses and reviews, the server re-checks. | A review in a browser is not a permission (Principle III). | The *reader* is one crate compiled twice, never two implementations. The server re-does authorization and bounds, not parsing. |
| **One large transaction** on commit (research §6). | All-or-nothing is worth more than streaming; a failure partway is the expensive case. | 049 FR-035's stated entry bound, refused before sending. Staging-then-swap is the escape hatch if measurement says the bound is too small. |
| **Two gates, one of them legal.** | The owner accepted this cost explicitly when choosing the merge. | Both sit in the last third: identity measurement before Phase 11, ADR-098 before Phase 15. Refusal or delay of either costs the phases after it, never the reader or the shelf. |
