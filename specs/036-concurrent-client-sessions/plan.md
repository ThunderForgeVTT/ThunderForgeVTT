# Implementation Plan: Concurrent Client Sessions

**Branch**: `036-concurrent-client-sessions` | **Date**: 2026-09-07 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/036-concurrent-client-sessions/spec.md`

## Summary

An account may hold several live sessions at once, exactly one of which is at
the play field; the rest are companion surfaces, of which the character sheet
is the first. The login-time eviction that made this impossible is removed and
what it protected is replaced by session visibility and explicit revocation. A
companion sheet can initiate a check the **system** declares, resolved by the
server on the existing authoritative roll path, and a companion may never
speak to a peer — peer continuation stays a play-field capability, and a
companion that has lost the server refuses and names the play field.

The approach follows two precedents already in the codebase rather than
inventing mechanisms:

- **The claim is a subscription-owned registry, not a row.** `peer_signaling.rs`
  already registers one live client connection for the duration of its stream,
  with the guard owned by the stream itself, precisely so an entry cannot
  outlive the connection. FR-030 ("a claim MUST NOT survive the client that
  made it") is the same requirement and takes the same shape.
- **A check is a manifest declaration, resolved server-side.** ADR-044 already
  makes the server the only party trusted to produce a roll, and spec 032 has
  already made a sheet something a system *declares* rather than something the
  app hard-codes. Sheet-initiated checks add one uniform declaration and one
  mutation; they add no rules to the client.

## Technical Context

**Language/Version**: Rust (2024 edition, server + engine), TypeScript 5 / React 18 (web)

**Primary Dependencies**: Axum, async-graphql, Diesel/PostgreSQL, tower-cookies; Bevy (wasm32) for the play field; Playwright for e2e

**Storage**: PostgreSQL. Existing `user_sessions` gains description columns; play-field claims are **not** stored (in-process registry, deliberately)

**Testing**: `cargo test --workspace -j 4` (native), `cargo clippy --target wasm32-unknown-unknown` (engine), `vitest` (web unit), Playwright via `scripts/e2e-parallel.mjs`

**Target Platform**: Chromium browsers only (constitution constraint); Linux server

**Project Type**: Web application — Rust backend, React shell, wasm engine, system packs

**Performance Goals**: A companion surface adds no canvas work; a play-field takeover resolves in one round trip; a sheet-initiated check resolves within the same envelope as `rollDice` today

**Constraints**: No product-code branch may exist for test-only providers (FR-023). Peer capability may not widen (FR-041). Session lifetime may not lengthen (FR-025)

**Scale/Scope**: Concurrent sessions bounded per account (10, LRU); one play-field claim per account; 8 system packs must keep working with no check declaration and 1 (dnd5e) gains one

## Constitution Check

*GATE: evaluated before Phase 0 and re-evaluated after Phase 1.*

| Principle | Verdict | Reasoning |
|---|---|---|
| **I. ECS owns simulation, React owns chrome** | PASS | A companion surface renders no canvas and holds no simulation state; it is React chrome over server data. The sheet's check button dispatches a mutation — it computes nothing. FR-031 makes "a companion does not run the canvas" a requirement rather than an accident. |
| **II. Plugin-modular engine architecture** | PASS | No new engine plugin, and no engine change beyond declining to mount without a play-field claim — a decision made by the web shell before the engine is created, not inside it. |
| **III. Ownership & authorization at the data boundary** | PASS | The claim, the roll and every companion read are adjudicated server-side. A sheet-initiated check enforces the same actor permission as any other actor action (`auth/actor_permissions.rs`). No client is trusted to say which client holds the play field. |
| **IV. Real ADRs before divergent implementation** | **ACTION REQUIRED** | Three architecturally significant decisions, each landing in the same change set: **ADR-073** concurrent sessions and the single play-field claim (supersedes the eviction policy at `auth/sessions.rs:370`); **ADR-074** system-declared checks and sheet-initiated rolls (a new manifest surface, extending ADR-044); **ADR-075** the peer boundary — peer capability belongs to the play field (amends ADR-052). |
| **V. Verify before claiming done** | PASS | Per-target checks are in the task plan: native `cargo test`/`clippy` for the server, `--target wasm32-unknown-unknown` for the engine, `tsc`/`vitest` for the web, and the e2e suite for every user story. |

**DMCA / content-moderation guardrail**: not engaged. Nothing here exposes one
world's compendium content outside that world; a companion surface shows the
same account the same world it already has.

**No violations to justify.** The Complexity Tracking table is empty by
design; the one structural addition (an in-process claim registry) is smaller
than the alternative it replaces, and is argued in research.md § R2.

## Project Structure

### Documentation (this feature)

```text
specs/036-concurrent-client-sessions/
├── plan.md              # This file
├── research.md          # Phase 0 — the eight decisions and what each rejected
├── data-model.md        # Phase 1 — entities, columns, lifetimes, transitions
├── quickstart.md        # Phase 1 — how to prove it, scenario by scenario
├── contracts/
│   ├── sessions.md      # Session listing, revocation, revoke-all
│   ├── play-field-claim.md  # Claim, takeover, demotion notice
│   ├── system-checks.md # The manifest `checks` declaration and `rollCheck`
│   └── e2e-fixtures.md  # The multi-client fixture and the OAuth stub
├── checklists/
│   └── requirements.md  # Written by /speckit-specify
└── tasks.md             # Phase 2 — written by /speckit-tasks, not here
```

### Source Code (repository root)

```text
src/server/src/
├── auth/
│   ├── sessions.rs            # eviction removed; description captured on create
│   ├── session_registry.rs    # NEW — sessions a person can see and end
│   └── oauth.rs               # unchanged by this feature; covered by e2e at last
├── play_field.rs              # NEW — the claim registry, owned by the stream
├── peer_signaling.rs          # registration now requires a held claim
├── graphql/
│   ├── mutations_sessions.rs  # NEW — endSession, endAllSessions; sessions query
│   ├── mutations_play_field.rs# NEW — claimPlayField, releasePlayField
│   ├── mutations_roll_check.rs# NEW — rollCheck, on the rollDice path
│   └── subscriptions.rs       # playFieldClaimChanged; peer gate
└── systems.rs                 # `checks` parsed off the manifest

crates/thunderforge-canvas-core/src/
└── system_rules.rs            # check declaration types, shared server/engine

packs/systems/
├── dnd5e/system.json          # gains `checks` (abilities + skills)
└── */system.json              # unchanged; a pack may declare none

apps/web/src/
├── pages/world/               # the play field claims on mount, releases on unmount
├── components/sheet/          # the check control; offers nothing when none declared
└── services/playFieldClaim.ts # NEW — claim state, demotion notice, "take it back"

apps/web/e2e/
├── fixtures/clients.ts        # NEW — a second client for an existing account
├── concurrent-sessions.spec.ts# NEW — US1, US4, and the eviction regression guard
├── companion-sheet.spec.ts    # NEW — US3a, US3b
├── companion-offline.spec.ts  # NEW — US3c
├── combat-panel.spec.ts       # NEW — US5
└── oauth-provider.spec.ts     # NEW — US6, against the harness stub

scripts/
└── e2e-parallel.mjs           # a stub-provider port per shard, as backends get one
```

**Structure Decision**: no new project or package. The server gains two
modules and three GraphQL surfaces; the web gains one service and a control on
an existing sheet; the harness gains one fixture and one stub service. The
check declaration lives in `thunderforge-canvas-core` because it is the crate
both the server and the engine already depend on and the only one whose tests
run natively — the same reasoning ADR-044 used for dice.

## Phase Summary

- **Phase 0 — research** (`research.md`): eight decisions, each with what it
  rejected. Complete; no NEEDS CLARIFICATION remains.
- **Phase 1 — design** (`data-model.md`, `contracts/`, `quickstart.md`):
  complete.
- **Phase 2 — tasks**: `/speckit-tasks`. Not written by this command.

## Complexity Tracking

No constitution violations. Table intentionally empty.
