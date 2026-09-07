# Implementation Plan: In-App Feedback, Delivered as GitHub Issues

**Branch**: `037-in-app-feedback` | **Date**: 2026-09-07 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/037-in-app-feedback/spec.md`

## Summary

A control on every screen collects a feature request, an issue report or a
general message, gathers the evidence the maintainer would otherwise have to
ask for — a bounded ring of browser logs, the screen and world, the versions,
and optionally a screenshot — shows the person **exactly** what will leave,
records the submission in the instance, and only then turns it into a GitHub
issue.

Three lines in the spec decide the shape, and the plan is mostly their
consequences:

- **FR-018 makes the instance the system of record.** The submission and its
  attachments are written before delivery is attempted, and delivery is a
  separate pass. That is what lets FR-030 be true — an instance with no GitHub
  configuration at all still collects feedback during a playtest. The pass
  itself is not new machinery: `src/server/src/lore_sync/schedule.rs` already
  solves "record now, deliver later, back off, do not duplicate" in the shape
  `src/app/src/main.rs` spawns five times, and this feature copies it rather
  than inventing a runner. See research.md § R3 and § R4.
- **FR-012 makes redaction a client-side capture-time filter.** A secret is
  removed *before a log line enters the buffer*, so the buffer the person
  reviews and the bytes that are submitted are the same array. The server runs
  the same rule set and **refuses** rather than rewriting, because an edit
  after approval means the person saw something other than what was sent —
  which is the promise FR-012 exists to keep, not the words it uses. See
  research.md § R6.
- **FR-023 through FR-025 make credentials a scope, not a copy.**
  `src/server/src/repo_host.rs` already resolves `SYNC_GITHUB_APP_*` with three
  private-key forms, a precedence rule and a base64 trap that needed a test to
  get right. It gains a scope parameter and a per-field resolution record;
  `GLOBAL_*` and `FEEDBACK_*` are two more prefixes through the same function,
  and `SYNC_*` keeps working because nothing about it changed. Spec 040 owns
  where an operator *sets* these; this plan owns only the resolver and the
  record of where each value came from, which is the shape 040's FR-021 needs.
  See research.md § R1.

Two smaller things follow the code rather than a preference: issue creation
already exists (`repo_host::open_issue`, and `issues:write` is already among
`github::REQUESTED_PERMISSIONS`), so nothing is added to
`crates/thunderforge-repo-host`; and a screenshot cannot come from the Bevy
canvas, because nothing sets `preserveDrawingBuffer` and the e2e suite has said
so since spec 001 — it comes from `getDisplayMedia`, whose picker is also a
better consent step than any checkbox this product could draw (§ R7).

## Technical Context

**Language/Version**: Rust (2024 edition, server), TypeScript 6 / React 19.2 (web)

**Primary Dependencies**: Axum, async-graphql, Diesel 2.3/PostgreSQL, `reqwest` (rustls), `thunderforge_repo_host` (`jsonwebtoken` 10 / `aws_lc_rs`), `aws-sdk-s3` + `aws-sdk-sts` against RustFS; Vite 8, react-router-dom 7, Radix `Dialog`, `graphql-ws`; Playwright for e2e. **No new dependency in either half.**

**Storage**: PostgreSQL for the submission, its attachment rows and its delivery attempts; RustFS for attachment bytes, under a `feedback/` key prefix that is never deduplicated (research.md § R8)

**Testing**: `cargo test --workspace -j 4` (native), `make lint` (lint-host + lint-wasm + file length), `pnpm --filter @thunderforge/web test` (vitest), Playwright via `scripts/e2e-parallel.mjs`

**Target Platform**: Chromium browsers only (constitution constraint — and `getDisplayMedia` is available there); Linux server

**Project Type**: Web application — Rust backend, React shell. **The engine is not touched at all.**

**Performance Goals**: SC-001's thirty seconds is a person's budget and never GitHub's — the mutation returns after a database write; the ring buffer is fixed-capacity so capture costs the same at hour four as at second four; the delivery pass is one indexed query every thirty seconds when there is nothing to do

**Constraints**: An issue body is capped at 65,536 characters (§ R15). Nothing in this codebase has ever deleted a stored object, and FR-016 requires that it now can — confined to a prefix nothing shares (§ R8). No credential, fragment or length may appear in any diagnostic (FR-027). The submitter's email may not reach the destination (FR-013)

**Scale/Scope**: One destination repository per instance. Log bundle bounded at 500 entries / 128 KB, screenshot at the existing `MAX_UPLOAD_BYTES`, 48 KB of log inline in the issue body and the whole bundle committed as a file. Attachment retention 30 days, independent of the tracker's copy

## Constitution Check

*GATE: evaluated before Phase 0 and re-evaluated after Phase 1.*

| Principle | Verdict | Reasoning |
|---|---|---|
| **I. ECS owns simulation, React owns chrome** | PASS | Nothing here is canvas state and nothing enters the engine. The screenshot is deliberately taken by the browser rather than by the engine (research.md § R7), which is the one place this feature could have leaked a product concern into the simulation crate and does not. The launcher, the review dialog and the log buffer are React chrome and a plain module. |
| **II. Plugin-modular engine architecture** | PASS | No engine plugin, no engine change, no engine build. `src/engine/` is untouched by every task in tasks.md. |
| **III. Ownership & authorization at the data boundary** | PASS | `submitFeedback` requires a session; `mySubmissions` filters on the caller's `user_id` and there is no field by which one account can read another's (contracts/feedback.md). Attachment bytes are served through an authenticated route like every other asset in this product. The new tables carry `created_by`/`updated_by` per Principle III and ADR-009. The game system is resolved server-side from `world_id` rather than accepted from the client. |
| **IV. Real ADRs before divergent implementation** | **ACTION REQUIRED** | Three architecturally significant decisions of this feature's own, each landing in the same change set: **ADR-084** the submission is the record and the tracker is a destination — the delivery pass, its backoff, and what "exactly once" can honestly mean against a host with no idempotency key; **ADR-085** redaction is a capture-time client filter and the server refuses rather than rewrites — a new trust boundary, and the one this feature would most plausibly get wrong; **ADR-086** feedback attachments are stored unshared so they can expire — the first object deletion in this codebase, and why `storage/dedupe.rs`'s warning is satisfied rather than ignored. A fourth, the DMCA determination, is **ADR-087** below. **Credential resolution is deliberately not among them** — see Numbering. |
| **V. Verify before claiming done** | PASS | Per-target checks are in the task plan: native `cargo test`/`clippy` for the server, `tsc`/`vitest` for the web, and an e2e spec per user story. No wasm target is involved because no engine code changes — stated so that a green host build is not mistaken for a partial check. |

**DMCA / content-moderation guardrail**: **engaged, and cleared.** This feature
publishes user-supplied content — a message, logs, and possibly a screenshot of
a world — to a repository that may be public. That is content leaving a world.
It is **not** the category the guardrail exists for: the checkpoint governs
making "one world's compendium content visible, copyable, searchable or
otherwise accessible outside that world", and nothing here exposes a
compendium, a lore entry, an actor or an item to anyone. What leaves is one
person's own deliberate report about their own screen, chosen by them, item by
item, after seeing it.

The relevant precedent is **ADR-067** ("a user-initiated mirror is not a public
repository"), whose reasoning transfers exactly: the instance is not operating
a repository of user content, it is carrying out one person's instruction about
one thing they chose to send. Two obligations follow and are requirements
rather than notes: FR-014's notice must be shown **before** submission and must
state that the destination is permanent and beyond the instance's recall
(§ R15 makes that literally true), and FR-010's review must let any part be
removed. **ADR-087 records this determination**, so that the next feature
pointing at a repository finds the reasoning rather than repeating it.

## Numbering, and one ADR this feature does not write

The highest ADR on disk is **072**. Four in-flight plans reserve above it, and
two of them collided while being written concurrently — spec 036 reserves
073–075, spec 040 reserves 076–080, and spec 039 reserves 080–083, so 039 and
040 both claim 080. **This feature therefore takes 084–087**, above every
existing reservation, and if any of those plans is renumbered these move with
whatever is left free. Stated so that a renumber is a find-and-replace and not
an archaeology exercise.

**The credential-resolution ADR is spec 040's, not this feature's.** 040's plan
already reserves it (its ADR-078, "credential resolution — scope outside,
source inside… generalises `SYNC_GITHUB_APP_*`"), and spec.md's Configuration
preamble puts credential collection, editing and the global-versus-specific
rule under 040. Writing a second ADR for the same decision is how two documents
begin disagreeing.

What this feature still *builds* is the resolver itself, because FR-023 to
FR-030 need it and 040 is not being built yet. `contracts/github-app-scopes.md`
is the statement of what feedback requires from it; whichever feature lands
first writes 040's ADR, and the other consumes it. T001 in `tasks.md` says so
explicitly rather than leaving it to whoever gets there.

## Project Structure

### Documentation (this feature)

```text
specs/037-in-app-feedback/
├── plan.md                    # This file
├── research.md                # Phase 0 — fifteen decisions and what each rejected
├── data-model.md              # Phase 1 — tables, states, bounds, retention
├── quickstart.md              # Phase 1 — how to prove it, scenario by scenario
├── contracts/
│   ├── feedback.md            # submitFeedback, mySubmissions, the review payload
│   ├── attachments.md         # the log bundle, the screenshot, bounds and redaction
│   ├── github-app-scopes.md   # FEEDBACK_*, GLOBAL_*, resolution, diagnostics
│   ├── delivery.md            # the pass, the issue, labels, idempotency, operator view
│   └── e2e-harness.md         # capture flags, the stub destination, seeded fixtures
├── checklists/
│   └── requirements.md        # Written by /speckit-specify
└── tasks.md                   # Phase 2
```

### Source Code (repository root)

```text
src/server/src/
├── repo_host.rs                    # AppScope, registration_for, per-field sources;
│                                   # open_issue gains a scope, labels, and a
│                                   # search-before-create; put_file added (R15);
│                                   # the git check leaves the shared resolver
├── feedback/
│   ├── mod.rs                      # NEW — record, states, retention constants
│   ├── redaction.rs                # NEW — the server's validator, refusing not rewriting
│   ├── issue_body.rs               # NEW — kind → title/labels/body, the 48 KB cut
│   ├── deliver.rs                  # NEW — one attempt: create, attach, adopt-on-retry
│   └── schedule.rs                 # NEW — TICK_SECONDS, backoff, due_now, spawn_*_task
├── storage/
│   ├── rustfs.rs                   # delete_object added, confined to feedback/
│   └── dedupe.rs                   # docs updated: deletion now exists, and where
├── graphql/
│   ├── mutations_feedback.rs       # NEW — submitFeedback, abandonFeedbackDelivery
│   ├── feedback_rate_limit.rs      # NEW — per account, on share_rate_limit's shape
│   └── queries/feedback.rs         # NEW — mySubmissions, undeliveredFeedback (admin)
├── graphql.rs                      # roots gain the two objects
├── schema.rs                       # regenerated
├── models.rs                       # FeedbackSubmission, FeedbackAttachment,
│                                   # FeedbackDeliveryAttempt, FeedbackDestination
└── assets_serve/feedback.rs        # NEW — authenticated attachment route

src/server/migrations/
└── 2026-09-07-010000-0000_feedback_submissions/{up,down}.sql   # NEW

src/app/src/
└── main.rs                         # spawn_feedback_delivery_task beside the
                                    # lore sync task; /feedback-assets/{id} route

config/
└── feedback-redaction.json         # NEW — the one rule list both sides read

apps/web/src/
├── services/
│   ├── feedbackLogBuffer.ts        # NEW — the bounded ring, redacted at capture
│   ├── feedbackRedaction.ts        # NEW — the rules, read from config/
│   ├── feedbackScreenshot.ts       # NEW — getDisplayMedia, one frame, stop
│   └── feedbackDraft.ts            # NEW — sessionStorage, cleared on success
├── components/feedback/
│   ├── FeedbackLauncher.tsx        # NEW — the control that is on every screen
│   ├── FeedbackDialog.tsx          # NEW — kind, fields per kind, the notice
│   ├── FeedbackReview.tsx          # NEW — everything that will be sent, removable
│   └── MySubmissions.tsx           # NEW — US5's list
├── api/feedback.ts                 # NEW — postGraphQL calls
├── main.tsx                        # the launcher and the buffer start here
└── vite.config.mts (apps/web/)     # a define block for the two versions

apps/web/e2e/
├── feedback-submit.spec.ts         # NEW — US1
├── feedback-evidence.spec.ts       # NEW — US2, including SC-004's planted secret
├── feedback-delivery.spec.ts       # NEW — US3, US6
├── feedback-credentials.spec.ts    # NEW — US4
├── feedback-status.spec.ts         # NEW — US5
└── fixtures/githubStub.ts          # NEW — the destination, per shard
```

**Structure Decision**: no new project, no new package, no new dependency. The
server gains one domain module (`feedback/`) beside `lore_sync/`, which is the
convention a new entity already follows in this codebase — domain rules in
their own directory, testable without GraphQL, with the resolvers thin above
them. `repo_host.rs` stays the only module in the product that speaks HTTP to
a repository host, per the boundary its own docs argue for. The web app gains
four services and one component directory; `feedbackLogBuffer` and
`FeedbackLauncher` are started from `src/main.tsx` because FR-001 says "any
screen" and `main.tsx` is the only place that is above every route — there is
no app-root overlay host today, and this feature adds the first one.

## Phase Summary

- **Phase 0 — research** (`research.md`): fifteen decisions, each with what it
  rejected. Complete; no NEEDS CLARIFICATION remains. The three that will be
  argued in review are § R6 (redaction before the review step), § R8 (the first
  object deletion) and § R15 (what "attached to the issue" can actually mean).
- **Phase 1 — design** (`data-model.md`, `contracts/`, `quickstart.md`):
  complete.
- **Phase 2 — tasks** (`tasks.md`): written, by user story, with an MVP slice.

## Complexity Tracking

No constitution violation. Two additions cost more than the requirement's
wording implies, and both are recorded here rather than discovered later.

| Addition | Why needed | Simpler alternative rejected because |
|---|---|---|
| `rustfs::delete_object` — the first object deletion in this codebase | FR-016 requires attachment retention to be *bounded*, and bounded means deletion. `storage/dedupe.rs` states that nothing deletes objects and that this is what makes shared paths safe | *Never delete, state retention as "indefinite"* is honest and makes FR-016 false — a screenshot of somebody's table kept forever because deletion was inconvenient is not a defensible privacy position. The risk `dedupe.rs` names is answered by construction rather than by care: feedback attachments are never deduplicated, so no second row can name the object being removed, and the prefix restriction is enforced inside the function rather than by its callers |
| An application version surface, which does not exist anywhere today | FR-009 names the application version as required context. `/api/status` deliberately reports no build identifier and is unauthenticated, so it cannot be the place | *Send the User-Agent and call it context* identifies the browser (which FR-009 asks for separately) and says nothing about which build is running — which is the question a bug report exists to answer |
