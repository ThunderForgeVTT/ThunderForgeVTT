# Implementation Plan: Player Onboarding — Invite-to-Actor Selection

**Branch**: `017-invite-actor-selection` | **Date**: 2026-08-23 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/017-invite-actor-selection/spec.md`

## Summary

A joining (non-GM) world member who has not yet claimed a PC-classified Actor is routed to a dedicated Actor Selection screen instead of the world dashboard. There they either claim a GM-designated "available" character or, if the world's new `allow_player_created_actors` setting is on, create their own (auto-claimed). The GM gets a per-Actor "available for claiming" toggle and an un-claim action, both server-enforced as Owner-level actions per the existing spec 010 ownership-block model. Session Setup also surfaces the same invite URL already generated from the world dashboard.

Technical approach: two new columns — `world_actors.available_for_claim` (bool) and a new `world_actor_claims` table (one-to-one `actor_id` ↔ `world_member_id`, world-scoped via the actor's own `world_id`) — plus `worlds.allow_player_created_actors` (bool, default false). A single atomic claim mutation (`UPDATE ... WHERE available_for_claim AND NOT EXISTS(claim)` inside a transaction, or a unique constraint + conflict-as-"already claimed") gives FR-006's concurrency guarantee without new locking primitives. A `myActorClaim(worldId)` query and a route-level redirect in the React app implement FR-001/002/003.

## Technical Context

**Language/Version**: Rust 1.75+ (server, edition 2024), TypeScript/React 18 (frontend)

**Primary Dependencies**: Axum, async-graphql, Diesel/Postgres (server); React Router, existing `api/*.ts` fetch-based GraphQL client pattern (frontend)

**Storage**: PostgreSQL — new `world_actor_claims` table, two new boolean columns (`world_actors.available_for_claim`, `worlds.allow_player_created_actors`)

**Testing**: `cargo test` (inline `#[tokio::test]` resolver tests, matching `mutations_actors.rs`/`mutations_moderation.rs` convention), Playwright e2e for the two-browser-context concurrent-claim race (FR-006)

**Target Platform**: Linux server (native only) — this feature has no Bevy/engine/wasm surface; it is pure GraphQL + React routing/UI

**Project Type**: Web application (existing `src/server` + `apps/web` structure)

**Performance Goals**: N/A beyond existing GraphQL request latency norms — no new performance-sensitive path

**Constraints**: The concurrent-claim guarantee (FR-006, SC-003) must be enforced at the database level (a unique constraint or `SELECT ... FOR UPDATE`), not just application-level check-then-write, since two requests can interleave between check and write

**Scale/Scope**: One new table, two new columns, ~6 new GraphQL operations (`setActorAvailability`, `claimActor`, `createAndClaimActor`, `unclaimActor`, `myActorClaim`, `availableActors`), one new React route (`/world/:id/actor-select`) plus a redirect gate on the existing world-entry route, one Session Setup UI addition (invite link display, reusing the existing invite-generation mutation)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS owns simulation)**: N/A — no canvas/engine state involved; Actor Selection is a pre-canvas onboarding screen. PASS.
- **Principle II (plugin-modular engine)**: N/A — no engine code touched. PASS.
- **Principle III (ownership/auth at the data boundary)**: `setActorAvailability`/`unclaimActor` require the caller to hold Owner-level access to the Actor (reuses `auth::actor_permissions::require_actor_permission`, spec 010's existing enforcement — no new authority concept, per FR-013/FR-004). `claimActor`/`createAndClaimActor` require the caller to be a non-GM world member with no existing claim in that world (server-side check, never client-trusted). PASS.
- **Principle IV (ADRs/specs before divergent implementation)**: This spec (017) exists; the two-column-plus-table storage design and the atomicity approach are documented in research.md. No new subsystem or replaced dependency — an ADR is not required (matches the bar set by specs 010/013, which did not need one; unlike 014's genuinely new shared-crate pattern). PASS, no ADR needed.
- **Principle V (verify before claiming done)**: `cargo check`/`cargo test` (native only — no wasm target for this feature) plus a live Playwright run per quickstart.md. Will be honored during implementation.

Initial gate: **PASS**, no violations to record in Complexity Tracking.

## Project Structure

### Documentation (this feature)

```text
specs/017-invite-actor-selection/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md         # Phase 1 output
├── quickstart.md         # Phase 1 output
├── contracts/
│   └── graphql-actor-claim.md
└── tasks.md              # Phase 2 output (/speckit-tasks — not created by this command)
```

### Source Code (repository root)

```text
src/server/
├── migrations/
│   └── <ts>_add_actor_claiming/{up,down}.sql   # world_actors.available_for_claim,
│                                                 # worlds.allow_player_created_actors,
│                                                 # new world_actor_claims table
├── src/
│   ├── models.rs                                # ActorClaim/NewActorClaim; available_for_claim
│   │                                             # + allow_player_created_actors on existing structs
│   ├── graphql/
│   │   ├── mutations_actor_claims.rs             # NEW: setActorAvailability, claimActor,
│   │   │                                         # createAndClaimActor, unclaimActor
│   │   ├── queries/actor.rs                      # + myActorClaim, availableActors (extend existing file)
│   │   ├── types.rs                               # GraphQLActorClaim, claim fields on GraphQLActor
│   │   └── mutations_invites.rs                   # no change (join_world untouched — claim
│   │                                               # gating happens client-side via routing + myActorClaim)
│   └── graphql.rs                                 # wire mutations_actor_claims into schema root

apps/web/
├── src/
│   ├── api/
│   │   └── actorClaims.ts                        # NEW: myActorClaim, availableActors,
│   │                                              # claimActor, createAndClaimActor,
│   │                                              # setActorAvailability, unclaimActor
│   ├── types/
│   │   └── actorClaim.ts                          # NEW
│   ├── pages/world/
│   │   └── ActorSelectionPage.tsx                 # NEW: /world/:id/actor-select
│   ├── routes/AppRoutes.tsx                        # + new route
│   ├── layouts/world-layout/
│   │   └── WorldStagingPage.tsx                    # + invite-link display (FR-015)
│   └── pages/world/actor/ActorDetailPage.tsx        # (or equivalent) + "Available for claiming"
│                                                     # toggle and claimed-by display for DM view
└── e2e/
    └── actor-claim.spec.ts                         # NEW
```

**Structure Decision**: Existing `src/server` + `apps/web` web-application layout, no new top-level project. Follows the same file-organization precedent as specs 010/013 (new `mutations_*.rs` file per feature, extend existing `queries/actor.rs` and `types.rs` rather than creating parallel actor-query files).

## Complexity Tracking

*No Constitution Check violations — table omitted.*
