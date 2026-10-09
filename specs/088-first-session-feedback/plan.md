# Implementation Plan: First Session Feedback

**Branch**: `088-first-session-feedback` | **Date**: 2026-10-09 | **Spec**: [spec.md](spec.md)

## Summary

Eight small stories from the first real session, each its own phase and
each proven by its own slice:

1. **World links** (P1). Link management moves to the players page, for
   the GM only. New links default to one use and a 7-day expiry, have
   128-bit codes, and give a distinct message for each refusal. Joining
   never creates an account, and the join pages are kept out of search
   indexes and `Referer` headers.
2. **Base maps** (P1). `createWorld` takes a `baseMapId`, and the Starting
   Scene is imported from a server-side base-maps directory, with the
   MBRound18 credit wherever the map shows.
3. **Actor view** and 4. **world page** (P2). Their own markup follows one
   breakpoint contract: one column under 768 px, two from 1024 px, three
   from 1536 px, capped at 1800 px.
5. **Clear rolls** (P2). A GM-only `clearWorldRolls` sets
   `worlds.rolls_cleared_at` and records event 39. `rolls/visibility.rs`
   withholds every roll at or before it, on every path. Nothing is deleted.
6. **Edge walls** (P2). A pure `perimeter_walls` adds the map's outer
   bounds as walls on import and on a background change, skipping what a
   file wall already covers, and replacing (not stacking) on a re-import.
7. **Mail form** (P3). One form over a pure form model. It sends only the
   keys that changed, and warns before a dirty form is lost.
8. **Hero polish** (P3). On top of `hotfix-player-hero-edit`: phone sizes,
   the viewer's card first, and a back link to the players page.

No runtime flags (spec.md, Decisions). Every write goes through GraphQL
and the world-event flow, except the existing REST map-import upload,
which already records `MAP_IMPORTED`.

## Technical Context

**Language/Version**: Rust (workspace edition and toolchain), TypeScript
with React 19, Vite and Tailwind v4.
**Primary Dependencies**: Axum, async-graphql, Diesel, `rand` (already in
the tree, for the CSPRNG), react-router-dom 7.18 (declarative). No new
dependency. Crockford base32 is 20 lines written here, not a crate
(research.md R3).
**Storage**: PostgreSQL. Three migrations:
- `world_invites.max_uses` nullable, `NULL` = no limit (the default);
- `canvas_image_assets.base_map_id TEXT NULL`;
- `worlds.rolls_cleared_at TIMESTAMPTZ NULL`.

The base maps are files in the server image, not rows. The RustFS bucket
holds each world's copy.
**Testing**: TDD unit tests for the pure cores (the link code, the clear
rule, the perimeter geometry, the settings form model, the demo mirrors),
`cargo test -p thunderforge-server`, vitest, and Playwright slices (spec.md,
Proof).
**Target Platform**: the Axum server on Linux, and Chromium.
**Project Type**: web application in a Cargo and pnpm monorepo.
**Performance Goals**: world creation with a map completes in under 5 s on
vtt-dev. A clear reaches every feed within 2 s (SC-005).
**Constraints**:
- Slices only, never the full suite.
- `components/ui/**`, `styles/**` and `routes/**` stay untouched for US3
  and US4 (FR-036).
- No client-side database. The world store and the GraphQL fetch hooks
  only.
- Telemetry attributes are bounded and anonymous (constitution VII).

**Scale/Scope**: about 40 files across the server, the web app and the
demo, plus three migrations, four guides, and `INSTANCE_CONFIGURATION.md`.

## Constitution Check

| Principle | How this plan meets it |
| --- | --- |
| I. ECS owns simulation, React owns chrome | The edge walls are wall rows, which the engine already draws and collides with. The credit line and the clear control are React chrome. The engine gains no code. |
| II. Plugin-modular engine | Not touched. |
| III. Ownership and authorization at the data boundary | Each new or changed resolver checks the caller's role per request (`runs_the_world` for links, the clear, and the walls a background adds). `worldByInviteCode` now needs a signed-in caller. The clear rule sits in `rolls/visibility.rs`, the one place a roll's visibility is decided. New rows carry `created_by`/`updated_by`, as their tables do. |
| IV. Specs before divergent implementation | This spec. Open items are the owner's decisions, each with a default. |
| V. Verify before claiming done | Each story ends on its slice. The pure cores are written test first. |
| VI. Every feature is proven by its own slice | The new e2e specs go into existing slices: accounts, worlds, scenes, canvas, actors, rolls, instance and hero-builder. `pnpm e2e:which --diff` names any others. `schema.graphql` and `auth/**` ask for the full suite, and the slices stand in for it (Open item 7). |
| VII. Telemetry is on, anonymous, redirectable | The events in contracts/telemetry.md carry outcomes and buckets only: no code, id, name or setting value. |

There are no violations.

## Worktree, branch and merge

- `git worktree add ../ThunderForgeVTT-088 -b 088-first-session-feedback main`,
  once the hotfixes below are on main.
- **Hotfix order.** The four hotfix branches land on main first:
  - US1 starts after `hotfix-world-permissions`. Its sign-in-only task
    (T028) also waits for `hotfix-invite-uses`, since both change
    `joinWorld` and `instance_access.rs`.
  - US8 starts after `hotfix-player-hero-edit`.
  - `hotfix-map-load-sync` is not a dependency. It is what carries a
    later import's walls to a second client.

  If a hotfix is not yet on main when its phase comes up, the phase waits,
  and the other phases go on.
- The worktree resolves `@thunderforge/engine` through main's
  `dist/engine`. 088 changes no engine code, so no wasm rebuild is needed.
- Before an e2e run on the external stack, wait on the e2e lock, and use
  `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1` with `--workers=1`.
- Commits are signed and go through `mcp__gitops__commit` with explicit
  files, one or more per task group.
- It reaches main by `merge_ff_only`. If main has moved, the owner decides
  whether to rebase.
- **Rollback.** Each story is its own commit range. The migrations are
  additive and have `down.sql`. `rolls_cleared_at` going away shows the
  hidden rolls again, which is safe.

## Project Structure

### Documentation (this feature)

```text
specs/088-first-session-feedback/
├── spec.md
├── plan.md
├── research.md            # facts with lines, decisions, alternatives
├── data-model.md          # migrations, the clear event, perimeter mark, form model
├── contracts/
│   ├── graphql.md         # every schema change, error codes
│   ├── base-maps.md       # the directory, maps.json, the HTTP routes, the credit
│   ├── layouts.md         # breakpoints and columns for the two pages
│   └── telemetry.md       # browser events and server counters for spec 086
├── quickstart.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/thunderforge-server/
├── migrations/2026-10-10-*_world_invites_default_one/
├── migrations/2026-10-10-*_base_map_assets/
├── migrations/2026-10-10-*_rolls_cleared_at/
└── src/
    ├── graphql/share_codes.rs           # 128-bit Crockford codes (US1)
    ├── graphql/mutations_invites.rs     # limits, refusal codes (US1)
    ├── graphql/queries/invite.rs        # sign-in required (US1)
    ├── auth/oauth.rs                    # sign-in-only from a join (US1)
    ├── base_maps/{mod,routes}.rs        # new: directory, baseMaps, HTTP (US2)
    ├── graphql/mutations_worlds.rs      # baseMapId, STARTING_MAP_FAILED (US2)
    ├── graphql/mutations_roll.rs        # clearWorldRolls; reveal and mutations_reroll.rs refuse (US5)
    ├── rolls/visibility.rs              # the clear rule (US5)
    ├── world_events.rs                  # EVENT_CODE_ROLLS_CLEARED = 39 (US5)
    ├── map_import/perimeter.rs          # new: perimeter_walls (US6)
    ├── map_import/{mod,offline}.rs      # wallEdges (US6)
    └── graphql/mutations_levels.rs      # wallEdges on a background (US6)
apps/thunderforge/src/main.rs            # --base-maps-dir, headers on /join and /invite
Dockerfile                               # base-maps stage
examples/maps/credit.json                # moved from apps/demo/credit.json
apps/web/src/
├── pages/world/players/PlayersPage.tsx (+ WorldLinksPanel.tsx)   # US1, US8
├── pages/world/JoinWorldPage.tsx, pages/auth/LoginPage.tsx       # US1
├── pages/world/CreateWorldPage.tsx (+ BaseMapPicker.tsx)         # US2
├── components/world/MapCredit.tsx                                # US2
├── pages/world/actor/ActorDetailPage.tsx                         # US3, US8
├── pages/world/WorldDashboardPage.tsx                            # US4
├── engine/world/sync/rolls.ts, hooks/useWorldRolls.ts            # US5
├── components/world/PlayDock/ChatPanel.tsx                       # US5
├── components/canvas-tools/MapImportTool/MapImportTool.tsx       # US6
├── pages/admin/components/{MailPanel,InstanceSettingsPanel}.tsx  # US7
├── pages/admin/settingsForm.ts, hooks/useUnsavedChanges.ts       # US7
└── e2e/*.spec.ts                                                 # one per story
apps/demo/src/backend/{handlers/dice.ts,events.ts,mapImport.ts}   # US5, US6
docs/guides/{inviting-players,your-first-world,rolls,doors-and-walls}.md
docs/INSTANCE_CONFIGURATION.md, .env.example, docs/CONTRIBUTING.md
```

## Complexity Tracking

None. The plan adds no crate and no abstraction layer beyond two pure
modules (the settings form model and `perimeter.rs`), each of which exists
to be tested alone. The base-maps directory is files, not a service.
