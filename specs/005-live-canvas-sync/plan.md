# Implementation Plan: Live Cross-Client Canvas Sync via GraphQL Subscriptions

**Branch**: `005-live-canvas-sync` | **Date**: 2026-08-20 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/005-live-canvas-sync/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

Research confirmed the spec's premise precisely: the server side of this feature is already fully built — `/api/ws` already upgrades to an authenticated `GraphQLWebSocket`, `SubscriptionRoot::world_events_created` already streams `world_events` filtered by `world_id`, and all four relevant event codes (wall/light/shape/token) are already emitted on every mutation. No client anywhere in `apps/web` has ever opened that WebSocket. This plan's real work is entirely client-side: add a minimal subscription transport, feed it into the four already-written (but never-fed) inbound consumer functions, add reconnect/backoff/full-resync behavior, and — the one genuine finding from research — close a real authorization gap in `world_events_created`, which today validates only that `world_id` is a well-formed UUID, not that the requester is actually a member of that world. That gap is harmless today (unused code) but becomes a real cross-world data leak the moment this feature makes the subscription load-bearing, so fixing it is a required part of this plan, not optional hardening.

## Addendum: User Story 4 (invite/membership fix), folded in post-planning

Folded into this spec after the initial plan/tasks were written, once independent unit-test and e2e gap analyses both converged on the same root cause spec 003 had already found live: no test anywhere in the project exercises a genuine non-owner account, because inviting one has never actually worked. Confirmed by direct code read:

- **Bug 1 (client)**: `apps/web/src/components/campaign/CampaignSettingsPanel.tsx`'s `handleGenerateInvite` (~line 154-165) calls `generateInviteCode(worldId: $worldId, maxUses: $maxUses)` with flat top-level arguments, but the resolver (`src/server/src/graphql/mutations_invites.rs:134`, `generate_invite_code(ctx, input: GenerateInviteCodeInput)`) expects a single `input` object wrapping both fields. Every invite-generation call from the UI fails today.
- **Bug 2 (server)**: even with Bug 1 fixed, `generate_invite_code` (`mutations_invites.rs:~154-160`) authorizes the caller by checking for an existing `world_members` row for `(world_id, user_id)` — but `create_world` (`src/server/src/graphql.rs:1182-1213`) never inserts one for the world's own owner. The world's own owner therefore fails their own world's invite-authorization check.

This addendum does not change this feature's Technical Context/Constitution Check below in substance — both fixes reuse existing patterns (correct an existing GraphQL call shape; insert one additional row in an existing transaction) and introduce no new subsystem, schema, or authorization mechanism. See Project Structure below for the two files this adds to scope.

## Technical Context

**Language/Version**: Rust 2024 edition (server crate native, unchanged), TypeScript/React (`apps/web`) — unchanged from specs 001-004.

**Primary Dependencies**: Existing stack only on the server (`async-graphql`/`async-graphql-axum`, already wired for subscriptions). One new client dependency: a minimal GraphQL-over-WebSocket client (e.g. `graphql-ws`'s client half) — the first real-time transport dependency `apps/web` has had client-side.

**Storage**: PostgreSQL — no schema change (research.md, data-model.md). The one server-side code change is an authorization check inside an existing resolver, not a schema or migration.

**Testing**: `cargo test` (server — new authorization test for `world_events_created`, per quickstart.md Scenario 3), Playwright (`apps/web/e2e/` — new two-browser-context live-sync spec, reconnect simulation via devtools offline mode), manual verification for network-drop scenarios that are awkward to fully automate.

**Target Platform**: Linux server + browser (unchanged).

**Project Type**: Web application — existing `src/server` / `apps/web` layout; this feature does not touch `src/engine` at all (no new Bevy plugin — it's a data-transport layer feeding the existing world store, same boundary walls/lights/shapes/tokens already use).

**Performance Goals**: Cross-client event delivery within the same few-seconds bar used throughout specs 001-004 (SC-001). Reconnect uses exponential backoff (research.md §5) rather than aggressive tight-loop retry.

**Constraints**: Constitution Principle III — the `world_events_created` authorization fix is a hard requirement of this plan, not a nice-to-have (research.md §2). Principle I — this feature only feeds data into the existing world store via the same `upsert_wall`/`upsert_light`/`upsert_shape`/`upsert_token` dispatch actions the outbound bridges already use (data-model.md); it introduces no new simulation-state ownership. Principle IV — the new client-side subscription transport is this codebase's first real-time client dependency and warrants an ADR (research.md §6).

**Scale/Scope**: Builds on and activates dormant infrastructure from specs 001 (wall/shape/light authoring, whose sync files already anticipated this transport) and an earlier, already-merged subscription-server phase (`graphql.rs`'s `SubscriptionRoot`, predating this feature). Also directly unblocks spec 004's own SC-002 (live cross-client token sync), per that spec's Assumptions section.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS owns simulation)**: PASS. This feature adds no new simulation state and no new React-side state store — inbound events dispatch into the existing world store via the same `upsert_*`/`remove_*` actions the outbound mutation bridges already use (research.md §5). The Bevy engine remains the single source of truth for what's drawn; this feature only supplies it fresher data, faster.
- **Principle II (Plugin-modular engine)**: N/A — this feature makes no `src/engine` changes at all. It is entirely a server-authorization-fix + `apps/web` client-transport feature.
- **Principle III (Ownership/authorization at the data boundary)**: FLAGGED — REQUIRED FIX, not yet applied. `world_events_created` currently only validates `world_id`'s format, not the requester's membership in that world (research.md §2). This MUST be corrected as part of this feature — activating a subscription this codebase has never actually served live traffic through, without this fix, would introduce a genuine cross-world data leak the moment real clients start calling it. This is called out explicitly so `/speckit-tasks` treats it as a blocking task, not an afterthought.
- **Principle IV (ADRs before divergent implementation)**: FLAGGED — RECOMMENDED. The new client-side subscription transport is `apps/web`'s first persistent, reconnecting, non-`fetch` network channel — an architecturally new category of client capability, even though low-risk and additive. An ADR should record the library choice, the cookie-based auth reuse (research.md §1/§3), and the full-refetch-on-reconnect resync strategy (research.md §4).
- **Principle V (Verify before done)**: Plan calls for native `cargo test` (server, including the new authorization test), `tsc`/build (web), and live Playwright + manual verification for the two-browser-context and network-drop scenarios (network-drop simulation is inherently harder to fully automate in CI than a straightforward mutation test, so manual quickstart verification is called out explicitly as a required, not optional, step per spec 003/004's precedent that live verification is not skippable when the premise is "does this actually work end-to-end").

**Gate result**: PASS, conditional on the Principle III authorization fix landing as part of this feature (not deferred) and the Principle IV ADR being authored alongside the transport implementation.

## Project Structure

### Documentation (this feature)

```text
specs/005-live-canvas-sync/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md         # Phase 1 output
├── contracts/
│   └── subscription-transport.md
└── tasks.md              # Phase 2 output (/speckit-tasks — not created by this command)
```

### Source Code (repository root)

```text
docs/adrs/
└── (new, recommended) <date>-0XX-graphql_subscription_client_transport.md   # Principle IV

src/server/src/
├── graphql.rs             # world_events_created (~line 1443-1500): add world-membership authorization check — the one required server change; create_world (~line 1182-1213): insert a world_members row for the owner (US4)
└── graphql/mutations_invites.rs  # generate_invite_code (~line 134-160): no server-side change needed here itself — the fix is the caller (below) and create_world (above); a test covering the previously-broken owner-invites-self path belongs here (US4)

apps/web/src/components/campaign/
└── CampaignSettingsPanel.tsx  # handleGenerateInvite (~line 154-165): fix the GraphQL call to send `input: { worldId, maxUses }` matching GenerateInviteCodeInput's actual shape (US4)

apps/web/src/
├── engine/world/sync/
│   ├── transport.ts        # (new) minimal subscription client: openWorldEventSubscription(worldId), reconnect/backoff, LiveSyncState
│   ├── walls.ts             # startWallEventSync gets fed the new transport's `events` — no internal logic change (FR-005)
│   ├── lights.ts            # same
│   ├── shapes.ts             # same
│   └── tokens.ts             # same (relevant now for today's upsert_token path; fully relevant once spec 004 lands)
└── pages/world/WorldPage.tsx # wires the transport's `state` to a "reconnecting" indicator; reconnect-triggered full re-fetch reuses the same loadWallsIntoStore/etc. functions spec 004's plan also touches (coordination note, research.md §4)

apps/web/e2e/
└── (new) live-sync.spec.ts  # US1-US2 two-browser-context verification
```

**Structure Decision**: Existing `src/server` / `apps/web` layout, unchanged. Unlike spec 004, this feature touches zero `src/engine` files — it is a transport-and-authorization feature living entirely in a couple of server files (small corrections) and a handful of `apps/web` sync/page/component files. Coordinate with spec 004 on `WorldPage.tsx` (both specs edit it, for different reasons — loading/error UI there vs. reconnect-triggered resync here). User Story 4 (invite/membership fix) is fully independent of the transport work — different files entirely (`graphql.rs`'s `create_world`, `mutations_invites.rs`, `CampaignSettingsPanel.tsx`) — and can be staffed in parallel with no coordination needed.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Principle III: authorization fix required before feature ships | Activating a previously-dormant, previously-harmless subscription resolver without adding the missing membership check would turn "unused code" into "live cross-world data leak" the moment this feature's client starts calling it | Shipping the transport first and fixing authorization later was considered and rejected — there is no safe window in which real client traffic flows through an unauthorized subscription |
| Principle IV: ADR recommended for new transport subsystem | First persistent/reconnecting client network channel in this codebase's history — a new category of capability, per the principle's own trigger condition | Skipping the ADR (as spec 003 correctly did) was considered but rejected — spec 003 introduced no new subsystem; this one does |
