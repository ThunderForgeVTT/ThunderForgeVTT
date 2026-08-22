# Implementation Plan: World Staging Route and Actor Ownership

**Branch**: `010-world-staging-actors` | **Date**: 2026-08-22 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/010-world-staging-actors/spec.md`

## Summary

Move "staging" out of `WorldPage.tsx`'s client-side `playView` UI-state (spec 009) into its own routed page, `/world/:id/staging`, reached from `/welcome`'s "Enter" link; `/world/:id/play` becomes canvas-only. Generalize actor ownership from a single non-null `world_actors.owned_by` column into a real permission model: a new `world_actor_permissions` table (Viewer/Editor/Owner per actor per member, DM always implicitly `Owner`, default `Viewer` for everyone else) that a DM-only "ownership block" UI edits from new dedicated routes, `/world/:id/actor/:actorId/{view,edit}`. Extend the existing token-drag authorization (already keyed on `tokens.owner_user_id`) to also honor actor-level `Owner` permission via the existing `tokens.actor_id` link, so live-play token control follows the new ownership block without a `tokens` schema change. Add actor sharing: a revocable, uncapped share link (`world_actor_shares`, modeled on `world_invites`) exposing a login-required, world-identity-scrubbed read-only preview, plus a "Copy to World" deep-clone mutation that duplicates the actor and all of its `world_actor_system_data` rows into a destination world the caller has DM-level access to, with zero live reference back to the source.

## Technical Context

**Language/Version**: Rust 1.75+ (server, `src/server`), TypeScript 5.x / React 18 (web, `apps/web`)

**Primary Dependencies**: Axum + async-graphql + Diesel/PostgreSQL (server); React Router, RxDB (`useWorldMembers`), the existing fantasy design system (`apps/web/src/components/ui/`) built on Radix primitives (web)

**Storage**: PostgreSQL — two new tables (`world_actor_permissions`, `world_actor_shares`); no columns added to any existing table (`world_actors.owned_by` is reinterpreted, not altered)

**Testing**: `cargo test` (server, native target), Playwright e2e (`apps/web/e2e/`), `tsc`/`vite build` (web)

**Target Platform**: Web (React SPA) + WASM (Bevy canvas engine — untouched by this feature; the canvas container is never mounted until `/play`, so the staging-route change carries zero engine-lifecycle risk, per research.md §1)

**Project Type**: Web application (existing `apps/web` frontend + `src/server` backend + `src/engine` WASM crate, unchanged split)

**Performance Goals**: No new performance target. The deep-copy mutation (`copySharedActorToWorld`) is a single-transaction, small-row-count operation (one actor row + a handful of `world_actor_system_data` rows) — no batching/async-job infrastructure is needed.

**Constraints**: All new authorization checks (actor create/edit, ownership-block edit, share-link create/revoke, copy) MUST be enforced server-side regardless of what the client shows/hides (Principle III) — the frontend's route guards (e.g., redirecting Viewer-only members away from `/edit`) are a UX convenience, never the actual gate.

**Scale/Scope**: One route relocation (`/staging` split out of `/play`), two new small tables, ~9 new GraphQL operations (2 queries + 1 field addition, 6 mutations across `actor-crud.md`/`actor-permissions.md`/`actor-share.md`), no new roles, no engine/WASM changes.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS owns simulation)**: PASS. No canvas simulation state changes hands. The one canvas-adjacent touchpoint — extending token-drag authorization (research.md §5) — is a server-side GraphQL mutation check, not a change to how the engine renders or moves tokens; the engine still just executes whatever move the (now slightly more permissive) mutation allows, exactly as it does today for `owner_user_id`-based drags.
- **Principle II (Plugin-modular engine)**: PASS / N/A. No `src/engine` changes at all — this feature's only canvas-adjacent change is server-side authorization logic in `mutations_tokens.rs`.
- **Principle III (Ownership & authorization at the data boundary)**: PASS, and this feature is largely *about* strengthening this principle for actors specifically — replacing a single-owner column with a real, server-enforced permission model (`require_actor_permission`, research.md §4), with every new mutation (`createActor`, `updateActor`, `setActorPermission`, `removeActorPermission`, `createActorShareLink`, `revokeActorShareLink`, `copySharedActorToWorld`) gated server-side and re-verified independently of any client-side state (contracts explicitly call this out, e.g. `copySharedActorToWorld` re-checking DM-level access rather than trusting `myDmWorlds`'s earlier read). New tables carry `created_by`-equivalent provenance (`world_actor_permissions` implicitly via the DM-only mutations that write it; `world_actor_shares.created_by` explicitly).
- **Principle IV (ADRs before divergent implementation)**: No new ADR required — this is schema-additive (two new small, purpose-built tables following the exact `world_invites` precedent) and route-additive, not a new subsystem, dependency, or an ownership-boundary *replacement* (it strengthens the existing boundary rather than crossing it); a Spec Kit spec already exists (`spec.md`) per this principle's requirement for net-new features.
- **Principle V (Verify before claiming done)**: Plan commits to `cargo check` (native, server) for all new Rust code, `tsc`/`vite build` for the web app, and live exercise of every user story in a running dev instance per `quickstart.md`, plus new Playwright coverage for the staging route split and the share/copy flow, and confirmation that existing canvas-authoring e2e coverage (wall/lighting/shape/map-import/asset-paste/token) is unaffected by `WorldPage.tsx`'s `playView`-state removal.

No violations. Complexity Tracking section is not needed.

## Project Structure

### Documentation (this feature)

```text
specs/010-world-staging-actors/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── actor-crud.md
│   ├── actor-permissions.md
│   └── actor-share.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
src/server/
├── migrations/
│   ├── <timestamp>_add_world_actor_permissions/{up,down}.sql   # NEW table (data-model.md)
│   └── <timestamp>_add_world_actor_shares/{up,down}.sql        # NEW table (data-model.md)
└── src/
    ├── schema.rs                          # add world_actor_permissions, world_actor_shares
    ├── models.rs                          # add ActorPermission, NewActorPermission, ActorShare, NewActorShare
    ├── auth/
    │   └── actor_permissions.rs           # NEW — require_actor_permission(), is_dm_of_world() (research.md §3, §4)
    └── graphql/
        ├── queries/
        │   ├── mod.rs                     # re-export the below
        │   ├── actor.rs                   # EXTEND — worldActors gains myPermissionLevel; add actorPermissions
        │   └── world.rs (or user.rs)      # add myDmWorlds (research.md §8)
        ├── mutations_actors.rs            # NEW — createActor, updateActor
        ├── mutations_actor_permissions.rs # NEW — setActorPermission, removeActorPermission
        ├── mutations_actor_shares.rs      # NEW — createActorShareLink, revokeActorShareLink,
        │                                  #   sharedActor (query, colocated for cohesion),
        │                                  #   copySharedActorToWorld
        ├── mutations_tokens.rs            # EXTEND — drag/move auth OR-condition (research.md §5)
        ├── mutations_invites.rs           # EXTEND — removeMember/leave-world path cascades
        │                                  #   world_actor_permissions cleanup (research.md §7)
        └── graphql.rs                     # merge new Query/Mutation structs into QueryRoot/MutationRoot

apps/web/src/
├── routes/
│   ├── pageLoaders.ts                     # add worldStaging, actorView, actorEdit, sharedActor
│   └── AppRoutes.tsx                      # add the four new routes (nested in MainLayout);
│                                           #   WorldPage's own /play route entry unchanged
├── pages/
│   ├── user/
│   │   └── WelcomePage.tsx                # "Enter" link target: /play → /staging
│   ├── world/
│   │   ├── WorldStagingRoutePage.tsx      # NEW — routed page wrapping the existing
│   │   │                                  #   layouts/world-layout/WorldStagingPage.tsx
│   │   │                                  #   presentational component (onPlay navigates
│   │   │                                  #   to /world/:id/play instead of local state)
│   │   ├── WorldPage.tsx                  # SIMPLIFIED — drop playView state, always render
│   │   │                                  #   full-screen WorldLayout shell directly
│   │   └── actor/
│   │       ├── ActorDetailPage.tsx        # NEW — shared view/edit page (mode prop),
│   │       │                              #   routes /actor/:actorId/{view,edit}
│   │       └── ActorOwnershipBlock.tsx    # NEW — DM-only permission editor, used inside
│   │                                      #   ActorDetailPage's edit mode
│   └── actor-share/
│       └── SharedActorPage.tsx            # NEW — /shared/actor/:code, read-only preview +
│                                           #   "Copy to World" flow (world picker, confirm,
│                                           #   success notice)
├── components/world/
│   └── NpcRoster/NpcRoster.tsx            # unchanged component, reused by the new staging route
├── api/
│   ├── actors.ts                          # EXTEND — createActor, updateActor,
│   │                                       #   actorPermissions, setActorPermission,
│   │                                       #   removeActorPermission
│   ├── actorShares.ts                     # NEW — createActorShareLink, revokeActorShareLink,
│   │                                       #   sharedActor, myDmWorlds, copySharedActorToWorld
│   └── world.ts                           # unchanged (myWorlds untouched per research.md §8)
└── types/
    ├── actor.ts                           # add ActorPermissionLevel, WorldActorRecord.myPermissionLevel
    └── actorShare.ts                      # NEW — SharedActorPreview, ActorShareLinkRecord

apps/web/e2e/
├── world-staging-route.spec.ts            # NEW — /welcome → /staging → /play flow, role gating
├── actor-ownership.spec.ts                # NEW — ownership-block editing, default-Viewer,
│                                           #   multi-Owner live-play control
└── actor-share.spec.ts                    # NEW — share link → preview → copy-to-world → independence
```

**Structure Decision**: Existing web-application split (`apps/web` React frontend, `src/server` Rust/GraphQL backend, `src/engine` Bevy/WASM — untouched by this feature) is unchanged. This feature is additive: two new small Postgres tables plus a handful of new backend modules following the exact file-per-domain precedent already set by `graphql/queries/actor.rs`/`graphql/mutations_invites.rs`, and a handful of new frontend pages/routes following the exact precedent set by spec 009's `WorldStagingPage.tsx`/spec's own `JoinWorldPage.tsx` (for the login-gated, world-membership-independent `SharedActorPage.tsx`).
