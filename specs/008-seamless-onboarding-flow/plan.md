# Implementation Plan: Seamless Sign-Up-to-Canvas Onboarding Flow

**Branch**: `008-seamless-onboarding-flow` | **Date**: 2026-08-21 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/008-seamless-onboarding-flow/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

Collapses the sign-up-to-canvas funnel from 2 forms + 1 modal + 5 navigations down to 2 forms + 0 modals + 0 dead-end stops for a zero-world user, and gives the WASM engine's silent load an honest staged status indicator. The two structural changes with the most leverage: (1) `create_world`'s server-side mutation resolver auto-inserts one default `Scene` row in the same DB transaction, so `WorldPage.tsx`'s existing scene-gating logic (`scenes.length > 0 || isSceneOwner`) already does the right thing with zero frontend changes — a fresh world simply never has zero scenes; (2) `/welcome` becomes a smart router that checks the user's existing-world count (reusing the already-existing `getMyWorlds()` query) and either renders the (now-honest) hub or redirects straight to `/worlds/create`, with no new routes and no change to `redirectAfterLogin`'s existing synchronous role-based logic in `AppRoutes.tsx`.

## Technical Context

**Language/Version**: Rust 2024 edition (`src/server`), TypeScript/React (`apps/web`) — unchanged.

**Primary Dependencies**: Existing stack only. No new dependency — the engine-load staged-status indicator is built from existing promise-chain instrumentation in `apps/web/src/engine/bevy/index.ts`/`useCanvasEngine.ts`, not a new progress-tracking library.

**Storage**: PostgreSQL, existing `worlds`/`scenes` tables — no schema change. `create_world`'s resolver gains a second insert (the default scene) wrapped in the same DB transaction as the world insert, using `create_scene`'s existing default values (`type: "battlemap"`, `grid_size: 5`, `grid_type: "square"`, `width`/`height: 100`) — no new columns, no new GraphQL response fields (the frontend already re-queries a world's scenes when entering `/world/:id/play`).

**Testing**: `cargo test` (server — new test confirming `create_world` always yields exactly one scene, and that a world-insert failure never leaves an orphaned scene or vice versa), `tsc`/`vite build` (web), Playwright (`apps/web/e2e/` — new spec covering the zero-world redirect, the fixed create-world form, the engine-load indicator, and the fixed invite-code CTA).

**Target Platform**: Linux server + browser (unchanged).

**Project Type**: Web application — existing `src/server` / `apps/web` layout. No `src/engine` (Bevy/WASM) changes — the engine itself is untouched; only the React-side loading *feedback* around it changes.

**Performance Goals**: Explicitly out of scope per spec.md's Assumptions — this feature changes what's shown during the engine's load, not the load's duration.

**Constraints**: Constitution Principle III — the default-scene insert reuses `create_scene`'s exact defaulting/DB-write pattern and inherits `create_world`'s existing `authenticated_user(ctx)` gate; no new authorization surface. Principle I (ECS owns simulation) is unaffected — a scene row existing before the canvas loads is a data-availability precondition, not new simulation logic.

**Scale/Scope**: Touches `src/server/src/graphql.rs` (`create_world`), `apps/web/src/pages/user/WelcomePage.tsx`, `apps/web/src/pages/world/CreateWorldPage.tsx`, `apps/web/src/pages/world/WorldPage.tsx` (one new conditional render block, mirroring the existing `scene-load-indicator` pattern), `apps/web/src/engine/bevy/index.ts`/`useCanvasEngine.ts` (staged status), and `apps/web/src/pages/auth/RegisterPage.tsx`/invite-code plumbing (FR-012). No `src/engine` changes.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS owns simulation)**: PASS. No engine/simulation logic changes — this feature only changes when scene *data* exists and how its absence-during-load is communicated in the React shell, never how the Bevy engine simulates or renders.
- **Principle II (Plugin-modular engine)**: N/A — no `src/engine` files touched.
- **Principle III (Ownership & authorization at the data boundary)**: PASS. The default-scene insert executes inside `create_world`'s existing `authenticated_user(ctx)`-gated resolver, using the same `auth_user.user_id` already used for the world's `created_by`/scene's `owner_id` — no new authorization path, no client-supplied trust decision.
- **Principle IV (Real ADRs and specs before divergent implementation)**: N/A — this is a UX/flow and single-resolver behavior change within an already-established mutation and table, not a new subsystem, dependency, or ownership boundary. No new ADR required (same reasoning as spec 006's Popover fix).
- **Principle V (Verify before done)**: Plan calls for `cargo check`/`cargo test` (server, new atomicity test for `create_world`+scene), `tsc`/`vite build` (web), and live Playwright verification of the full zero-world and returning-user funnels, the engine-load indicator, and the invite-code fix before this feature is reported done.

**Gate result**: PASS, no violations, no Complexity Tracking entries needed.

## Project Structure

### Documentation (this feature)

```text
specs/008-seamless-onboarding-flow/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md         # Phase 1 output
├── contracts/
│   └── create-world-mutation.md   # Phase 1 output — extended create_world contract
└── quickstart.md         # Phase 1 output
```

### Source Code (repository root)

```text
src/server/src/
└── graphql.rs             # create_world resolver: wraps world insert + default scene
                            # insert in one DB transaction, reusing create_scene's
                            # existing default values (no new input/output fields)

apps/web/src/
├── pages/user/WelcomePage.tsx       # becomes a smart router: queries getMyWorlds()
│                                    # on mount, redirects to /worlds/create if empty
│                                    # (replace navigation, no extra click), otherwise
│                                    # renders the hub with fixed invite-code entry
│                                    # (FR-007) and correct copy (FR-008/FR-009)
├── pages/world/CreateWorldPage.tsx  # removes the two dead Select dropdowns
│                                    # (game-system, interface-pack) and their state;
│                                    # on success, navigates to /world/${id}/play
│                                    # instead of the dashboard
├── pages/world/WorldPage.tsx        # one new conditional render block
│                                    # (!engineReady && !engineError), mirroring the
│                                    # existing scene-load-indicator/-error pattern,
│                                    # showing engine load stage text
├── engine/bevy/index.ts             # mountEngine/getWasmModule gain an optional
│                                    # stage-callback param ("downloading" →
│                                    # "starting") — no new dependency, just
│                                    # instrumenting the existing await points
├── engine/bevy/useCanvasEngine.ts   # exposes the new stage as part of its result
│                                    # alongside existing engineReady/error
├── pages/auth/RegisterPage.tsx      # FR-012: preserves a pending invite code
│                                    # (from query param, already how /join/:code's
│                                    # "register instead" link would need to pass it)
│                                    # across registration and returns to redemption
└── pages/world/WorldDashboardPage.tsx  # FR-006: dead placeholder panels
                                        # (Actors/Tokens/Events/Game system/Interface
                                        # pack) removed or wired to real (if sparse)
                                        # data; Scenes panel reflects the real scene
                                        # list. Screen itself is unchanged in
                                        # structure — only reachable later, not at
                                        # creation time.
```

**Structure Decision**: Existing `src/server` / `apps/web` layout, unchanged. No new routes in `AppRoutes.tsx` — `/welcome`'s existing route just renders smarter content; `/worlds/create` and `/world/:id/play` already exist. No new backend tables/columns.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

No violations — table intentionally omitted.
