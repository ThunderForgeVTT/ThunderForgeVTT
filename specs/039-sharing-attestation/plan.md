# Implementation Plan: The Sharing Attestation — Recorded, Enforced, and on Every Path That Publishes

**Branch**: `039-sharing-attestation` | **Date**: 2026-09-07 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/039-sharing-attestation/spec.md`

**Not legal advice.** `legal/README.md` marks the instance's prose as needing
review before launch and nothing in this plan changes that. This document
decides how the product behaves. Whether the words are the right words remains
a lawyer's judgement, and three of the decisions below *require* the words to
change — which is named as a review item, not slipped through as an edit.

## Summary

The position already exists in prose and the notice-and-takedown program that
enforces it already runs. What is missing is a record, a boundary, and reach.
This feature adds those three and nothing else.

The approach follows four things the codebase already does, rather than
inventing mechanisms:

- **The terms are compiled in and archived on sight.** `admin.rs:37` already
  does `include_str!("../../../config/realm-defaults.json")` and says why: a
  file read at runtime "can be absent at exactly the moment it is needed, which
  is the first boot, on someone else's machine". The sharing terms take the
  same route into the server, their version is the hash of their own words, and
  the server writes the full text into `terms_versions` at startup — so a
  version that anyone could ever attest to was archived *before* the
  attestation existed. FR-016 is then a property of the order things happen in,
  not of a retention policy somebody has to honour.
- **The attestation has no foreign keys.** `content_moderation_actions` already
  carries an `account_id` with no FK, and its migration says why in as many
  words: history must survive the deletion of the world, the account and the
  entity. FR-010 asks for exactly that and gets exactly that shape.
- **The gate is one function on all four paths.** Four
  `create_*_share_link_impl` functions already exist with near-identical
  signatures. Each gains one argument and one call, and an SDL guard test
  asserts that *every* mutation whose name matches `create*ShareLink` carries
  it — which is how FR-002 covers a fifth content type without a fifth
  decision.
- **Time-based work uses the shape `main.rs` already uses four times.**
  `lore_sync/schedule.rs:108` names it: "a `spawn_*_task` in the library,
  called from the binary, owning its own schedule and staying off every hot
  path. No new infrastructure, deliberately." The termination sweep is the
  fifth. Everything it does is also reachable lazily — on the disabled
  account's own sign-in, and from the admin review page — so the tick is a
  backstop rather than the mechanism.

**One decision here is expensive and cannot be avoided.** FR-023 requires a
takedown to reach copies already adopted into other worlds, and
`collections/copy.rs` was built so that it cannot: "FR-012 forbids any
referential link back to the source. So the copies carry no source id, and the
receipt this returns is **not stored**." Reaching adopted copies means reversing
that, and reversing it re-opens ADR-069's determination. That is ADR-094, it is
the DMCA guardrail's business, and research.md § R6 argues it in full.

## Technical Context

**Language/Version**: Rust (2024 edition, server), TypeScript 5 / React 18 (web). No engine change.

**Primary Dependencies**: Axum, async-graphql, Diesel/PostgreSQL; `sha2` (already in the tree via the asset content-hash work) for the terms version identity; Playwright for e2e

**Storage**: PostgreSQL. Four new tables (`terms_versions`, `attestations`, `content_adoptions`, `account_terminations`, `account_notices` — five, counting notices), one new column on `content_moderation_actions` (`parent_case_id`), no change to any content table

**Testing**: `cargo test --workspace -j 4` (native), `make lint` (lint-host + lint-wasm), `pnpm --filter @thunderforge/web test`, Playwright via `scripts/e2e-parallel.mjs`

**Target Platform**: Chromium browsers only (constitution constraint); Linux server

**Project Type**: Web application — Rust backend, React shell. The Bevy engine is untouched by this feature end to end

**Performance Goals**: The gate adds one hash comparison and one insert to a publish, which happens at human speed. The termination sweep is off every hot path and ticks at 300s. Takedown fan-out is bounded by the adoption graph, which is bounded by `MAX_MEMBERS = 100` per copy

**Constraints**: No new user-facing query may list adoptions, by anybody, for any reason — ADR-069's determination rests on non-enumerability and ADR-094 keeps that intact (research.md § R6). Refusals may not disclose a valid version identity (FR-013). The existing repeat-infringer threshold, lookback and counting are reused unchanged (FR-027)

**Scale/Scope**: Four publishing paths today and a structural guard for the fifth; one legal document becomes server-side and gains a version; three legal documents need a lawyer's eye for reasons this plan creates

## Constitution Check

*GATE: evaluated before Phase 0 and re-evaluated after Phase 1.*

| Principle | Verdict | Reasoning |
|---|---|---|
| **I. ECS owns simulation, React owns chrome** | PASS | Nothing here touches the canvas. The attestation surface is a dialog over server-provided text; the standing page is a read of server-derived state. No React component holds any of it as truth — the version identity the client sends back came from the server one request earlier and is re-checked there. |
| **II. Plugin-modular engine architecture** | PASS | No engine plugin, no engine module, no engine change. `cargo clippy --target wasm32-unknown-unknown` is run in the task plan to prove that rather than assert it. |
| **III. Ownership & authorization at the data boundary** | PASS, and this feature is an instance of it | FR-011/FR-014 are Principle III applied to an agreement. The gate is a server-side function called inside the same transaction that mints the share code, not a component that renders a dialog. The disabled-account allowlist is a positive list at `graphql/helpers.rs`, so a mutation added later is refused by default rather than reachable by omission. New tables carry `created_by`-equivalent provenance where they have an actor (`attestations.subject_user_id`, `content_adoptions.adopted_by`, `account_terminations.closed_by`); the three that must outlive their subject carry it **without a foreign key**, following the precedent `content_moderation_actions` set and documented. |
| **IV. Real ADRs before divergent implementation** | **ACTION REQUIRED** | Four architecturally significant decisions, all landing in the same change set. **ADR-093** — the attestation record: terms compiled into the server, versioned by content hash, archived at startup. **ADR-094** — adoption provenance and the reach of a takedown: reverses spec 026 FR-012's "no referential link", amends ADR-069, and carries the guardrail determination below. **ADR-095** — account standing and the termination window: standing derived from the existing counting, the window recorded, the sweep on the existing `spawn_*_task` shape. **ADR-096** — the operator acknowledgement: an instance attesting on the same record as a person. |
| **V. Verify before claiming done** | PASS | Per-target checks are in the task plan: native `cargo test`/`clippy` for the server, `--target wasm32-unknown-unknown` for the engine (to prove it was not disturbed), `tsc`/`vitest` for the web, and an e2e spec per user story. Every claim in this plan about existing code was read out of the file before it was written down. |

**Numbering — settled 2026-09-09.** This paragraph predicted the collision and
it happened: 036 landed 073–075, 037 landed 084–087, 040 landed 088–093, 041
landed 081–083 plus 094. That left **076–080 free**, and spec 039 takes
**076–079**:

| ADR | Subject |
|---|---|
| 076 | The sharing attestation record |
| 077 | Account standing and the termination window |
| 078 | The operator acknowledgement |
| 079 | Adoption provenance and the reach of a takedown (**the gate**) |

The task list said 093–096 and this paragraph said 080–083; both are superseded
by the table above.

### DMCA / content-moderation guardrail — engaged, and answered

The constitution requires two things on record before implementation begins
for any feature that makes one world's content accessible outside that world.
This feature makes nothing newly accessible — it restricts four paths that are
already open and adds none — but **ADR-094 creates a cross-world record of
which content came from which content**, and ADR-069's determination reasoned
partly from the absence of exactly that. So the checkpoint is engaged on its
merits, not as a formality.

**(a) The programme is operational.** Confirmed by reading it, not by citation.
`src/server/src/moderation/` implements intake with statutory-element
validation, disable, counter-notice, forwarding with a waiting period, lazy
auto-restoration and repeat-infringer counting; `mutations_moderation.rs` and
`queries/moderation.rs` expose it; `apps/web/e2e/dmca-takedown.spec.ts` and
`dmca-counter-notice.spec.ts` drive it end to end. Two gaps found while
reading, both recorded in research.md § R11 and fixed by this plan or by a
task: `collections/mod.rs`'s `"scene" => None`, and a wrong entity-type string
in `lore_sync` that silently disables nothing.

**(b) The determination.** A `content_adoptions` record is **not** a
centralized public repository, and the reasoning has to be about the properties
ADR-069 actually relied on rather than about the word "repository":

- It is **not readable by any user, ever.** No query, field, subscription or
  route exposes it. It is read by `moderation::reach` and by nothing else.
- It is **not enumerable in either direction** even for an administrator: there
  is no "what came from this" listing and no "what did this world take"
  listing. The only access is a bounded walk from one entity id that a notice
  already named — that is, from content somebody has already accused.
- It **indexes nothing that was not already published.** A copy exists because
  a person with a link chose to make it. The record says the copy happened; it
  does not make any content reachable by anyone who could not already reach it.
- It **exists solely to make a takedown effective**, which is the opposite of
  the exposure ADR-069 weighed. A determination that forbade it would be a
  determination that the safest posture is one where a notice cannot be
  honoured.

**This is a determination this plan proposes; it is not yet accepted.** ADR-069
was accepted by MBRound18 as accountable owner, with a stated risk accepted on
the record. ADR-094 needs the same signature before T-numbers in Phase 2 begin,
and the task plan makes that a blocking task rather than a note.

**The cost is real and named**: spec 026's FR-012 said a copy is independent.
After ADR-094 a copy is independent *to its adopter* and traceable *to
moderation*. Those are different claims and the second one is new. If the
accountable owner declines it, FR-023 through FR-023d are not buildable and
must be withdrawn from the spec — see research.md § R6, which states that
outcome as a real option rather than a rhetorical one.

## Project Structure

### Documentation (this feature)

```text
specs/039-sharing-attestation/
├── plan.md              # This file
├── research.md          # Phase 0 — twelve decisions and what each rejected
├── data-model.md        # Phase 1 — tables, derivations, lifetimes, transitions
├── quickstart.md        # Phase 1 — how to prove it, scenario by scenario
├── contracts/
│   ├── attestation.md            # The terms query, the attestation input, the record
│   ├── publishing-gate.md        # What every publishing path must now do
│   ├── takedown-reach.md         # Adoption provenance and the fan-out
│   ├── standing-and-termination.md # Strikes, the ladder, the window, the appeal
│   └── operator-acknowledgement.md # First-run acknowledgement and the notice contact
├── checklists/
│   └── requirements.md  # Written by /speckit-specify
└── tasks.md             # Phase 2
```

### Source Code (repository root)

```text
legal/
├── sharing-terms.md               # RENAMED from collection-sharing-terms.md, and reworded
│                                  #   to cover four paths, not one (research.md § R3)
├── operator-responsibilities.md   # NEW — the one piece of net-new prose (research.md § R12)
├── terms-of-service.md            # unchanged here; its [OPERATOR] markers are spec 040's
└── README.md                      # gains the two new rows and the versioning note

src/server/src/
├── legal/
│   └── mod.rs                     # NEW — include_str! of legal/, section split, version hash
├── attestation.rs                 # NEW — record, retrieve, redact; used by shares and setup
├── publishing.rs                  # NEW — PublishableKind and the one gate every path calls
├── notices.rs                     # NEW — durable in-app notices; mail is spec 040's
├── moderation/
│   ├── mod.rs                     # unchanged primitives; gains strike_count()
│   ├── reach.rs                   # NEW — walk content_adoptions, fan a case out and back
│   └── standing.rs                # NEW — derived standing, the window, the sweep task
├── collections/
│   └── copy.rs                    # records an adoption per copied entity, in the same txn
├── graphql/
│   ├── mutations_collection_shares.rs # gains the attestation argument and the gate call
│   ├── mutations_actor_shares.rs      # same
│   ├── mutations_item_shares.rs       # same
│   ├── mutations_ability_shares.rs    # same
│   ├── mutations_moderation.rs        # takedown and counter-notice fan out through reach.rs
│   ├── mutations_standing.rs          # NEW — fileAppeal, resolveAppeal
│   ├── queries/legal.rs               # NEW — sharingTerms, operatorStatement
│   ├── queries/standing.rs            # NEW — myStanding, accountStanding (admin)
│   └── helpers.rs                     # the disabled-account allowlist lives here
├── users/mod.rs                   # export filled in; deletion respects other people's worlds
└── auth/admin_setup.rs            # setup will not complete without the acknowledgement

src/app/src/main.rs                # ensure_terms_versions_recorded(); spawn_standing_task()

src/server/migrations/
├── 2026-09-07-130000-0000_terms_versions_and_attestations/
├── 2026-09-07-131000-0000_content_adoptions/
├── 2026-09-07-132000-0000_moderation_parent_case/
├── 2026-09-07-133000-0000_account_terminations/
└── 2026-09-07-134000-0000_account_notices/

apps/web/src/
├── components/legal/AttestationDialog.tsx  # NEW — one dialog, four callers
├── pages/world-collections/WorldCollectionsPage.tsx  # inline terms → the shared dialog
├── pages/world/actor/ActorDetailPage.tsx             # gains the dialog it never had
├── pages/world/item/ItemDetailPage.tsx               # same
├── pages/world/ability/AbilityDetailPage.tsx         # same
├── pages/user/StandingPage.tsx             # NEW — strikes, notices, download, appeal
└── pages/admin/ModerationReviewPage.tsx    # flags become terminations an admin can act on

apps/web/e2e/
├── sharing-attestation.spec.ts    # NEW — US1, US2, US3, US4
├── takedown-reach.spec.ts         # NEW — US6
├── account-standing.spec.ts       # NEW — US5, US7
├── operator-acknowledgement.spec.ts # NEW — US8
└── user-data-export.spec.ts       # its known-gap pinning test is rewritten, not deleted
```

**Structure Decision**: no new project, package or crate. The server gains four
small modules and two moderation submodules; the web gains one dialog reused by
four pages and one account page. `src/server/src/legal/` is a module rather
than a constant in `attestation.rs` because two documents live there and a
third (spec 040's) will, and because the section-splitting logic is the Rust
twin of `apps/web/src/legal/legalDocuments.ts` and should be reviewable beside
a name that says so. Nothing is added to `crates/` — none of this is shared
with the engine, and pretending otherwise would put legal prose in a crate that
compiles to wasm.

## Phase Summary

- **Phase 0 — research** (`research.md`): twelve decisions, each recording what
  the codebase does today, what was chosen and what was rejected. Complete; no
  NEEDS CLARIFICATION remains. Three of them (§ R3, § R6, § R12) end in a
  change to legal prose and say so.
- **Phase 1 — design** (`data-model.md`, `contracts/`, `quickstart.md`):
  complete.
- **Phase 2 — tasks** (`tasks.md`): written, by user story, with an MVP slice.

## Complexity Tracking

The Constitution Check records one ACTION REQUIRED (four ADRs) and no
violations of a principle. Two structural additions are larger than the default
and are justified here rather than buried in research.

| Addition | Why needed | Simpler alternative rejected because |
|---|---|---|
| `content_adoptions` — a stored link from every copy back to its source | FR-022/FR-023: a takedown that leaves live copies has not happened, and `copy.rs` deliberately keeps nothing that could find them | *Match by content hash.* Only reaches copies of an uploaded **file**; the shared `asset_id` already covers that case by accident. Text — a transcribed rules block in a lore entry or an ability description — is the case the spec was written about, and hashing it is fingerprinting, which spec 039 puts out of scope in as many words. *Store the copy receipt as-is.* The receipt names a whole collection and every record made from it, which is a richer link than the one needed and closer to the enumerable index ADR-069 argued against. The adoption record is one pair of ids per entity and is readable only by moderation. |
| `spawn_standing_task` — a fifth periodic task in `main.rs` | FR-036: deletion happens at the end of the window. Every other time-based rule in this codebase is evaluated lazily on a read, and a deleted account is precisely the thing nobody reads — the lazy shape has no reader here and would silently never fire | *Purely lazy, on the account's own sign-in.* Rewards never signing in, which is what somebody avoiding deletion would do. *An external cron hitting an admin route.* Works, and makes correct behaviour depend on an operator's crontab; a self-hosted instance that never sets one is an instance where the window is a lie. The sweep function is nonetheless callable from the admin surface and from sign-in, so the tick is a backstop and the behaviour is testable without waiting for one. Its default is **not to delete**: `MODERATION_TERMINATION_REQUIRES_HUMAN` defaults true (FR-040), so the shipped behaviour is a queue an administrator acts on, and automatic deletion is a thing an operator turns on. |
