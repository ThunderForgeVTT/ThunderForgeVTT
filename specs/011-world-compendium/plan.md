# Implementation Plan: World Compendium

**Branch**: `011-world-compendium` | **Date**: 2026-08-22 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/011-world-compendium/spec.md`

## Summary

Relocate spec 010's NPC catalog off the Session Setup (staging) page onto a new, dedicated `/world/:id/compendium` route with a tabbed shell (NPCs real; Items/Abilities placeholder "coming soon"), and add a row-select/right-side preview panel to the NPCs tab so a DM or player can inspect an NPC's detail without leaving the table or navigating to the full actor edit screen. Session Setup shrinks to exactly Play + Players + a new "Last Session Notes" panel, backed by one new nullable `worlds.session_notes` column and a DM/GM-only mutation to update it. No new actor data model, no new permission concept — this is a UI relocation plus one small new field.

## Technical Context

**Language/Version**: Rust 1.75+ (server, `src/server`), TypeScript 5.x / React 18 (web, `apps/web`)

**Primary Dependencies**: Axum + async-graphql + Diesel/PostgreSQL (server); React Router, the existing fantasy design system's `Tabs`/`Card`/`Panel` primitives (`apps/web/src/components/ui/`), the FlexSearch-backed `@/search/actorSearch` module and `NpcCatalog`/`getWorldActors` from spec 010 (web)

**Storage**: PostgreSQL — one new nullable column, `worlds.session_notes TEXT`; no new tables (the NPCs tab reuses `world_actors` as-is)

**Testing**: `cargo test` (server, native target), Playwright e2e (`apps/web/e2e/`), `tsc`/`vite build` (web)

**Target Platform**: Web (React SPA). No engine/WASM involvement — the Compendium never mounts the canvas, matching Session Setup's existing non-canvas footprint.

**Project Type**: Web application (existing `apps/web` frontend + `src/server` backend, unchanged split; `src/engine` untouched)

**Performance Goals**: No new performance target. The NPCs tab's search/filter is already instant (client-side FlexSearch index built in spec 010); the preview panel is a pure client-side render of already-fetched data, no extra round trip per selection.

**Constraints**: The Last Session Notes mutation MUST enforce DM/GM-only write server-side regardless of what the client's UI shows/hides (Principle III) — mirroring the exact pattern already used for the NPC-add control's `isGm` gate. The preview panel's Edit action MUST only be offered when `myPermissionLevel` (already returned per-actor from spec 010) is Editor or Owner; the server-side `updateActor` check is unchanged and remains the real gate.

**Scale/Scope**: One new route (`/compendium`), one relocated feature (NPC catalog, unchanged capability), one new small preview-panel component, one new DB column + one new query field + one new mutation. No new tables, no new roles, no engine changes.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS owns simulation)**: PASS / N/A. Nothing in this feature touches canvas/engine state — the Compendium is a pure React+GraphQL surface, exactly like Session Setup already is.
- **Principle II (Plugin-modular engine)**: PASS / N/A. No `src/engine` changes.
- **Principle III (Ownership & authorization at the data boundary)**: PASS. The one new mutation (`updateWorldSessionNotes`) is DM/GM-only, enforced server-side using the same `isGm`-equivalent check (`world.createdBy`/`world_members` role, matching `useWorldRole`'s existing fallback) already established for actor creation in spec 010 — no new authorization pattern is introduced. The NPCs tab's create/edit/view gating reuses spec 010's `require_actor_permission`/`is_dm_of_world` checks unchanged; this feature adds zero new server-side authorization surface for actors.
- **Principle IV (ADRs before divergent implementation)**: No new ADR required — this is a UI relocation plus one additive nullable column following the exact single-field-addition precedent already used for `world_actors.description` (spec 010's follow-up work), not a new subsystem or an ownership-boundary change. This spec (`spec.md`) satisfies the net-new-feature documentation requirement.
- **Principle V (Verify before claiming done)**: Plan commits to `cargo test` (server) and `tsc`/`vite build` (web) for all new code, live exercise of every user story in a running dev instance per `quickstart.md`, and new/updated Playwright coverage confirming Session Setup no longer shows the NPC catalog/Lore placeholder while the Compendium's NPCs tab reproduces that same capability.

No violations. Complexity Tracking section is not needed.

## Project Structure

### Documentation (this feature)

```text
specs/011-world-compendium/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── compendium-npcs.md
│   └── session-notes.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
src/server/
├── migrations/
│   └── <timestamp>_add_world_session_notes/{up,down}.sql   # NEW nullable worlds.session_notes column
└── src/
    ├── schema.rs                      # add session_notes to worlds table!
    ├── models.rs                      # add session_notes: Option<String> to World
    └── graphql/
        ├── types.rs                   # GraphQLWorld gains sessionNotes field
        └── graphql.rs                 # WorldMutation gains update_world_session_notes
                                        #   (DM/GM-only, reuses is_dm_of_world-equivalent check)

apps/web/src/
├── routes/
│   ├── pageLoaders.ts                 # add worldCompendium
│   └── AppRoutes.tsx                  # add /world/:id/compendium route (nested in MainLayout,
│                                       #   same guard pattern as /staging)
├── pages/world/
│   ├── WorldCompendiumRoutePage.tsx   # NEW — routed page wrapping the new presentational
│   │                                  #   WorldCompendiumPage component
│   └── compendium/
│       ├── WorldCompendiumPage.tsx    # NEW — tabbed shell (NPCs/Items/Abilities), owns
│       │                              #   selected-row state, renders NpcCompendiumTab +
│       │                              #   ActorPreviewPanel side by side
│       ├── NpcCompendiumTab.tsx       # NEW — search + table (adapted from
│       │                              #   components/world/NpcCatalog, but rows select
│       │                              #   instead of navigating away)
│       ├── ActorPreviewPanel.tsx      # NEW — right-side detail panel: name, description,
│       │                              #   classification, type, View/Edit actions
│       │                              #   (Edit gated on myPermissionLevel)
│       └── ComingSoonTab.tsx          # NEW — shared placeholder for Items/Abilities
├── layouts/world-layout/
│   └── WorldStagingPage.tsx           # SIMPLIFIED — remove the NPC panel + Lore placeholder,
│                                      #   add SessionNotesPanel; keep Play + Players
├── components/world/
│   ├── NpcCatalog/NpcCatalog.tsx      # unchanged; superseded on Session Setup by the new
│   │                                  #   NpcCompendiumTab but left in place (still a valid,
│   │                                  #   simpler standalone component if reused elsewhere)
│   └── SessionNotesPanel/
│       └── SessionNotesPanel.tsx      # NEW — read-only text for players, editable
│                                      #   textarea + Save for DM/GM
├── api/
│   ├── actors.ts                      # unchanged (getWorldActors/createActor/updateActor
│   │                                  #   already sufficient for the NPCs tab)
│   └── world.ts                       # EXTEND — updateWorldSessionNotes; getWorld's
│                                      #   existing query gains sessionNotes field
└── types/
    └── world.ts                       # WorldRecord gains sessionNotes: string | null

apps/web/e2e/
├── world-compendium.spec.ts           # NEW — tabbed shell, NPCs tab search + row-select
│                                      #   preview, Items/Abilities placeholders, DM-only
│                                      #   add-NPC control
└── session-notes.spec.ts              # NEW — DM edits/saves notes, persists across reload,
                                       #   Player sees read-only text, Session Setup no
                                       #   longer shows NPC list/Lore placeholder
```

**Structure Decision**: Existing web-application split (`apps/web` React frontend, `src/server` Rust/GraphQL backend, `src/engine` untouched) is unchanged. This feature is additive/relocational: one new nullable column, one new query field, one new mutation, and a new `pages/world/compendium/` directory of frontend components following the exact precedent already set by spec 010's `pages/world/actor/` directory (a feature-scoped subfolder of small, single-purpose components) — no new backend module files are needed since the changes are small enough to live in the existing `types.rs`/`graphql.rs` alongside the pre-existing `WorldMutation`.
