# Implementation Plan: GM Staging Page and Full-Screen Play Canvas

**Branch**: `009-gm-staging-page` | **Date**: 2026-08-21 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/009-gm-staging-page/spec.md`

## Summary

Replace `WorldLayout.tsx`'s permanent placeholder shell at `/world/:id/play` with a two-state UI: a staging page (scene selector, real player roster, real NPC roster, extension point for lore) that every world member sees first, and a full-screen canvas mode reached via a "Play" action, with an on-screen toggleable sidebar (scenes, NPC/combat, trackers/settings, lore extension point) replacing the old permanent sidebar. GM-only controls (scene creation, NPC roster editing) are gated by the caller's role; non-GM members see the same shell read-only. Staging↔full-screen is a per-user, in-place UI state change — no new route, no cross-user sync, and the canvas container must stay permanently mounted across that toggle (not conditionally rendered) so the already-booted Bevy/WASM engine's canvas handle stays valid. The one real backend gap closed: a `worldActors(worldId)` GraphQL query, since `world_actors` (with its existing `is_npc` flag) has no read path today.

## Technical Context

**Language/Version**: Rust 1.75+ (server, `src/server`), TypeScript 5.x / React 18 (web, `apps/web`)

**Primary Dependencies**: Axum + async-graphql + Diesel/PostgreSQL (server); React Router, RxDB (world-member/invite replication), the existing fantasy design system (`apps/web/src/components/ui/`) built on Radix primitives (web)

**Storage**: PostgreSQL — no schema change; reuses existing `world_actors`, `world_members`, `scenes` tables

**Testing**: `cargo test` (server, native target), Playwright e2e (`apps/web/e2e/`), `tsc`/`vite build` (web)

**Target Platform**: Web (React SPA) + WASM (Bevy canvas engine, unaffected by this feature's own code — only its surrounding chrome changes)

**Project Type**: Web application (existing `apps/web` frontend + `src/server` backend + `src/engine` WASM crate, unchanged split)

**Performance Goals**: No new performance target beyond "no regression" — the full-screen canvas toggle must not cause the ~190MB WASM engine to re-download or re-initialize (state.started/module reuse already exists in `apps/web/src/engine/bevy/index.ts`; this feature's layout change must not break that by unmounting the canvas container)

**Constraints**: The canvas container element (`#game-canvas-container`) MUST remain mounted in the DOM across the staging↔full-screen toggle — Bevy's `module.start(canvasSelector)` only runs once per page load (`state.started` guard) and does not re-attach to a replacement DOM node, so conditionally rendering the container away and back would leave the running engine pointed at a detached/removed element

**Scale/Scope**: One frontend route's layout/chrome (`/world/:id/play`), one new narrow GraphQL query, no new tables, no new roles

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS owns simulation)**: PASS. This feature only changes the React chrome around the canvas (staging page, sidebar, full-screen toggle). No canvas simulation state (tokens, walls, selection) moves into React; the canvas container itself is preserved verbatim, just repositioned by CSS/layout.
- **Principle II (Plugin-modular engine)**: PASS / N/A. No engine (Bevy/`src/engine`) changes are needed — full-screen mode is purely a host-page layout change around the existing canvas mount point.
- **Principle III (Ownership & authorization at the data boundary)**: PASS, with one addition to verify: the new `worldActors(worldId)` query MUST enforce the same visibility rule already used by `scenes(worldId)` (`require_visible_world`), and GM-only mutations this page surfaces (scene creation) already enforce ownership server-side — this feature adds no new mutation, only gates existing GM-only actions in the UI, with the server remaining authoritative as it already is.
- **Principle IV (ADRs before divergent implementation)**: No new ADR required. This is a layout/chrome restructuring of `WorldLayout.tsx`/`WorldPage.tsx` plus one new read-only query that follows an established pattern (`scenes(worldId)` + `require_visible_world`) exactly — not a new subsystem, ownership boundary change, or dependency replacement.
- **Principle V (Verify before claiming done)**: Plan commits to `cargo check` (native, server) for the new query, `tsc`/`vite build` for the web app, and live exercise of both staging page and full-screen mode in a running dev instance, plus Playwright coverage of the toggle and role-gating.

No violations. Complexity Tracking section is not needed.

## Project Structure

### Documentation (this feature)

```text
specs/009-gm-staging-page/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
src/server/src/
├── schema.rs                          # unchanged — world_actors already has is_npc
├── models.rs                          # unchanged — WorldActor already has needed fields
└── graphql/
    ├── queries/
    │   ├── mod.rs                     # add `pub mod actor;` + re-export ActorQuery
    │   └── actor.rs                   # NEW: worldActors(worldId) query
    └── graphql.rs                     # merge ActorQuery into QueryRoot

apps/web/src/
├── layouts/
│   └── world-layout/
│       ├── WorldLayout.tsx            # REPLACED — becomes the full-screen canvas chrome
│       │                              #   (on-screen back control + toggleable sidebar),
│       │                              #   canvas container stays permanently mounted
│       └── WorldStagingPage.tsx       # NEW — staging page (scenes/players/NPCs/lore stub)
├── pages/world/
│   └── WorldPage.tsx                  # gains a local "staging" | "playing" UI-state toggle;
│                                       #   renders WorldStagingPage or the full-screen
│                                       #   WorldLayout around the same always-mounted canvas
│                                       #   container
├── components/world/
│   ├── SceneSwitcher/                 # unchanged — reused in both staging and sidebar
│   └── NpcRoster/                     # NEW — renders worldActors(worldId) with is_npc,
│                                       #   used by both staging page and sidebar
├── api/
│   └── actors.ts                      # NEW — getWorldActors(worldId) GraphQL client
├── types/
│   └── actor.ts                       # NEW — WorldActorRecord type
└── hooks/
    └── useWorldMembers.ts             # unchanged — reused for player roster (RxDB-backed)

apps/web/e2e/
└── gm-staging-page.spec.ts            # NEW — staging↔full-screen toggle, role gating,
                                        #   NPC roster display, engine-state preservation
```

**Structure Decision**: Existing web-application split (`apps/web` React frontend, `src/server` Rust/GraphQL backend, `src/engine` Bevy/WASM — untouched by this feature) is unchanged. This feature is additive within `apps/web` (one new page component, one new roster component, one new API/type pair) plus one new narrow backend query module (`graphql/queries/actor.rs`), following the exact file-organization precedent already set by `graphql/queries/scene.rs` and `graphql/queries/invite.rs`.
