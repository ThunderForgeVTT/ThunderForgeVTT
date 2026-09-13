# Implementation Plan: Pausing a World's Play

**Branch**: `051-operator-playfield-shutdown` (built on `claude/inspiring-bohr-dc3c91`) | **Date**: 2026-09-13 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/051-operator-playfield-shutdown/spec.md`

## Summary

An operator pauses one world's live play. Within five seconds every live stream
for that world ends with a `WORLD_PLAY_PAUSED` signal, every browser in it lands
on a calm notice, and the server refuses every path back in, including
reconnects, direct calls and offline changes, until an operator lifts the pause.
A takedown on content in a world being played raises a request instead of
pausing, and an operator approves or declines it.

The approach reuses what already works rather than inventing a kill switch:

- **Reaching the table**: the five-second database poll that already ends
  revoked sessions' streams (`session_lifetime.rs`) learns to end a paused
  world's streams too, and a world event makes the honest case sub-second
  (research R1).
- **Holding the lock**: an explicit gate at every world-scoped write and
  play-starting entry point, made closed by a surface test that fails on any
  unclassified root field. This is the pattern `admin_surface_tests` and
  `disabled_surface_tests` already use (R2).
- **Asking first**: the first operator request table, with one pending request
  per world and a conditional update so two operators cannot both decide (R4).

Six phases, each provable on its own.

| Phase | What lands | How it is proved |
|---|---|---|
| 1 | ADR-100; the migration; the `play_pause` module with the gate, pause and lift | Server tests: gate refuses, blind to `is_admin`; one active pause per world; lift is final |
| 2 | **Streams end** (`until_stream_must_end`, event 28), **operator page** with an immediate pause, the **notice** | e2e scenario 1: two browsers on different scenes removed within 5 s, a third world untouched |
| 3 | **The lock holds**: gate on every world-scoped root field and REST write, the surface test, `heartbeat`/`reconcile` `PlayPaused`, `/api/events` removed | e2e scenario 2, including the severed link and direct calls; the surface test |
| 4 | **Requests**: `world_live_play`, the takedown hook, approve and decline | e2e scenario 3 |
| 5 | **Lifting and knowing**: lift, `worldPlayState`, the world page's *that and when*, the operator record | e2e scenario 4, and SC-004's inspection |
| 6 | Polish: axe audit, the room-screen check, SC-001 measured, heartbeat comment amended, docs | Measurements recorded in `quickstart.md`'s scenarios |

## Technical Context

**Language/Version**: Rust 2024 edition (server, `thunderforge-cache-core`); TypeScript 5 / React 18 (`apps/web`)

**Primary Dependencies**: Axum, async-graphql (subscriptions over `graphql-ws`), Diesel + PostgreSQL, tokio; `graphql-ws` client; Playwright. **New dev dependency**: `@axe-core/playwright` in `apps/web` (research, contract *The notice*)

**Storage**: PostgreSQL: four new tables (`world_play_pauses`, `world_play_pause_requests`, `world_play_pause_triggers`, `world_live_play`), two enums, one world event code ([data-model.md](data-model.md))

**Testing**: `cargo test` (server unit and integration against the test DB), Playwright e2e in Chromium, `tsc --noEmit`

**Target Platform**: Linux server; Chromium clients (constitution)

**Project Type**: Web service with a web client

**Performance Goals**: A paused world's clients are off the playfield within **5 s** (SC-001); the stream poll stays at one query per stream per 5 s (no increase over today's session poll)

**Constraints**: Must work across server processes (no in-process-only signal); must not revoke logins; the gate must fail closed and the stream poll fail open (R1); no erasure of client-held content is promised

**Scale/Scope**: One world at a time; ~30 world-scoped root mutations and 4 subscriptions to classify and gate; one new admin page, one notice page, a world-page banner

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Verdict | Notes |
|---|---|---|
| **I. ECS owns simulation, React owns chrome** | Pass | No simulation changes. The notice and admin page are React chrome. On a pause the page tears the engine down as it does on leaving play. |
| **II. Plugin-modular engine** | Pass | No new engine capability. If teardown needs an engine call it uses the existing leave-play path. |
| **III. Authorization at the data boundary** | Pass, and it is the point of the feature | The gate is server-side at every GraphQL and REST entry point, the surface test makes the list closed, and all new tables carry `created_by`/`updated_by`. The deliberate exception (the gate ignoring the site-admin short-circuit) *narrows* authority and is recorded in ADR-100. |
| **IV. ADRs and specs before divergent implementation** | Pass with ADR-100 drafted | Operators gaining a power no world role can override changes an authority boundary. [ADR-100](../../docs/adrs/20260913-100-an_operator_can_pause_a_worlds_play.md) was accepted by the owner on 2026-09-13. |
| **V. Verify before claiming done** | Pass | Per phase: `cargo test` for the server, `tsc --noEmit` for the web app (`pnpm verify` does not type-check it), the phase's e2e run in a real browser. No engine files are expected; if one changes, `cargo check --target wasm32-unknown-unknown`. |
| **DMCA / Content Moderation Guardrail** | Not engaged | Nothing becomes reachable outside a world. The feature strengthens condition (a), the takedown programme being effective, by reaching a live table. |

**Post-design re-check (after Phase 1 artifacts)**: still passing. Design added no
new project, no new canvas library, and no client-side authority. One item to
watch in Phase 3: the gate's list is long, and a resolver that forgets the call
is caught only if the surface test's second half actually calls it. The task
list must make that second half enumerate `GATED` mechanically, not by hand.

## Project Structure

### Documentation (this feature)

```text
specs/051-operator-playfield-shutdown/
├── spec.md
├── plan.md              # this file
├── research.md          # R1–R9
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── graphql.md       # member and operator fields, errors
│   └── live-play-lock.md # the gate, the closed list, streams, client signals, the notice
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
docs/adrs/20260913-100-an_operator_can_pause_a_worlds_play.md   # new, Proposed

src/server/
├── migrations/<timestamp>_pausing_a_worlds_play/{up,down}.sql  # new
└── src/
    ├── play_pause/                     # new module
    │   ├── mod.rs                      # pause, lift, record reads
    │   ├── gate.rs                     # refuse_if_paused, refuse_scene_if_paused
    │   ├── requests.rs                 # raise, attach trigger, decide
    │   ├── live_play.rs                # world_live_play mark and read
    │   └── *_tests.rs
    ├── graphql/
    │   ├── session_lifetime.rs         # until_stream_must_end
    │   ├── subscriptions.rs            # wrap worldEventsCreated, playersOnline
    │   ├── mutations_play_field.rs     # gate + wrap playField
    │   ├── mutations_heartbeat.rs      # gate; mark world_live_play
    │   ├── mutations_reconcile.rs      # PlayPaused
    │   ├── mutations_play_pause.rs     # new: operator mutations
    │   ├── queries/play_pause.rs       # new: operator queries, worldPlayState
    │   ├── play_pause_surface_tests.rs # new: the closed list
    │   ├── admin_surface_tests.rs      # classify the new operator fields
    │   └── mutations_*.rs, queries/world_sync_plan.rs, world_events_since.rs  # gate calls
    ├── peer_signaling/surface.rs       # gate + wrap peerSignals
    ├── graphql/mutations_moderation.rs # raise_for_takedown after the takedown's work
    ├── moderation/reach.rs             # hand fan-out worlds to the hook
    ├── world_events.rs                 # EVENT_CODE_WORLD_PLAY_PAUSED = 28
    ├── network/ws.rs, session.rs       # /api/events handler removed (R7)
    └── schema.rs
src/app/src/main.rs                     # drop the /events route; REST write handlers gated

crates/thunderforge-cache-core/src/queue.rs   # RejectionReason::PlayPaused

apps/web/
├── src/
│   ├── api/playPause.ts                # new
│   ├── api/graphqlClient.ts            # route WORLD_PLAY_PAUSED centrally
│   ├── engine/world/sync/heartbeat.ts  # paused ≠ offline
│   ├── engine/world/sync/offlineQueue.ts # PlayPaused → revert and count
│   ├── engine/world/sync/subscriptionClient.ts # stream error code → paused
│   ├── pages/world/PlayPausedPage.tsx  # new: the notice
│   ├── pages/world/WorldPage.tsx       # event 28 → notice; banner
│   ├── pages/admin/PlayPausesPage.tsx  # new: requests, active pauses, record, pause a world
│   ├── pages/admin/components/adminSections.ts
│   └── routes/AppRoutes.tsx
└── e2e/
    ├── play-pause.spec.ts
    ├── play-pause-holds.spec.ts
    ├── play-pause-request.spec.ts
    ├── play-pause-lift.spec.ts
    └── fixtures/playPause.ts           # pause/lift/decide via GraphQL; shared takedown filing
```

**Structure Decision**: Everything lives in the existing server crate, web app
and one cache crate. The only new module is `src/server/src/play_pause/`, kept
separate from `moderation/` because a pause is independent of a takedown
(FR-042), and the separation is what makes "restoring content does not call into
pauses" checkable by reading the imports.

## Phase notes the task list must carry

- **ADR-100 accepted before Phase 2.** It is the authority change; Phase 1's
  migration and gate are reversible, Phase 2 puts the lever in an operator's
  hands.
- **Phase 3 makes the gate list mechanical.** The surface test's second half
  iterates `GATED`, builds a minimal valid call per field from a fixture table,
  and fails when a field lacks a fixture, so a new gated field cannot be listed
  without being exercised.
- **The takedown hook stays non-fatal** (R4). A test forces the hook to fail and
  asserts the takedown still took effect and the failure was logged with the
  action id.
- **Declined leaves no trace** is proved by what the member query cannot select,
  and by an e2e that asserts no world event and no history entry after a
  decline.
- **No `git add -A`** while agents run; stage explicit paths.

## Complexity Tracking

No constitution violations to justify.

One deliberate trade recorded instead: the lock is a GraphQL/REST gate with a
closed-list test, not a database trigger (research R2). The trigger would be
the stronger backstop, and it was rejected because many play tables reach their
world only through a scene, and a trigger would also block world deletion. It
also cannot gate streams, so the GraphQL gate is needed anyway. ADR-100 names it
as the next step if the surface test is ever found to have missed a path.
