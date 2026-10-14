# Implementation Plan: Clocks

**Branch**: `095-clocks` | **Date**: 2026-10-13 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `specs/095-clocks/spec.md`

## Summary

A system declares its clock kinds in its manifest (`clocks` block, the shape
of 029's `resources` and 067's `conditions`); the server stores clocks in a
`world_clocks` table owned by a world and optionally a scene, exposes them
through five mutations and two queries gated on `runs_the_world()`, and
announces every change as `WorldEvent` 43 with per-viewer reach keyed on
`shown`; the web store gains `clocks`, a bridge and an event sync like every
other synced kind; the play dock mounts a Clocks tab only for systems that
declared kinds. React draws the ring; the engine learns nothing.

## Technical Context

**Language/Version**: Rust (workspace edition, stable), TypeScript 5 /
React 18 / Vite.

**Primary Dependencies**: `thunderforge-pack-system-spec` (manifest block),
`thunderforge-server` (Diesel, async-graphql, `world_events`),
`thunderforge-authz` (`runs_the_world`), the web world store and its sync
modules. No new crates, no new npm packages.

**Storage**: PostgreSQL, one new table `world_clocks` (Diesel migration,
`created_by`/`updated_by` per Constitution III). NOTIFY/LISTEN as it is.

**Testing**:

- `cargo test -p thunderforge-pack-system-spec`: the `clocks` block's
  validator, field-name parity test against the engine-side type if one is
  added (none is: the engine does not read clocks).
- `cargo test -p thunderforge-server`: migration, model, mutation gates
  (member / GM / paused), clamp in SQL, concurrent ticks, reach filter for
  code 43, `worldClocks` filtering by `shown`.
- Vitest in `apps/web`: reducer commands, `applyClockWorldEvent`, the
  bridge's optimistic tick and rollback, `useDockSections` mounting the tab
  only with kinds.
- Playwright: `apps/web/e2e/clocks-systems.spec.ts`,
  `clocks-tick.spec.ts`, `clocks-hidden.spec.ts` in the `clocks` slice.

**Target Platform**: Linux server; Chromium/Firefox/WebKit.

**Project Type**: web service + web app. The wasm engine is untouched.

**Performance Goals**: SC-002, a tick on the player's dock within 2 s on a
local stack. The event carries an id only; one fetch per event.

**Constraints**: no client-side database (AGENTS.md). Components dispatch
into the world store; only the bridge sends GraphQL. No system named in
host code (FR-005). Hidden clocks absent on the wire (FR-008, FR-013).
Rust files stay under 1000 lines (`scripts/check-file-length.sh`).

**Scale/Scope**: one manifest block, one migration, one table, one event
code, five mutations, two queries, one store slice, one sync module, one
API module, one dock panel, one dock section, two manifest edits, three e2e
specs, one slice.

## Constitution Check

| Principle | How this plan meets it |
| --- | --- |
| I. ECS owns simulation, React owns chrome | Clocks are chrome: a dock panel, no engine plugin, no sync message to Bevy. |
| II. Plugin-modular engine | Nothing is added to the engine. Kinds are data in the manifest (ADR-062); the host understands no system. |
| III. Authz at the data boundary | Every mutation checks `runs_the_world()` server-side; `worldClocks` filters by `shown` server-side; `world_clocks` carries `created_by`/`updated_by`. |
| IV. ADR + spec before divergent implementation | This spec; no ADR needed, the clock reuses ADR-000's event flow and ADR-062's data-not-code stance. |
| V. Verify per target | Server and pack-spec crates by `cargo test` on the host; web by Vitest; no wasm change, so no wasm lint delta. |
| VI. Proven by its own slice | Slice `clocks`; own specs `clocks-systems.spec.ts`, `clocks-tick.spec.ts`, `clocks-hidden.spec.ts` (the `clocks-` prefix); neighbours `status-systems.spec.ts` (the manifest-block seam, 029) and `genie-puzzle-clock-rewards.spec.ts` (Genie's own clocks stay as they were). The gate before merge is that slice plus every slice `pnpm e2e:which --diff` names (the owner's decision, 2026-10-07). |
| VII. Telemetry on, anonymous | The existing request tracing covers the new mutations; no new attribute carries a label or a user. |

No violations.

## Project Structure

### Documentation (this feature)

```text
specs/095-clocks/
├── spec.md
├── plan.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/thunderforge-pack-system-spec/src/
├── lib.rs                      # `clocks: Option<Vec<clocks::SystemClockKind>>`, validator call
└── clocks.rs                   # SystemClockKind, clocks_from_manifest, validate_clocks_content

crates/thunderforge-server/
├── migrations/2026-10-13-120000-0000_world_clocks/{up,down}.sql
└── src/
    ├── schema.rs               # world_clocks (generated)
    ├── models.rs               # WorldClock, NewWorldClock
    ├── world_events.rs         # EVENT_CODE_CLOCK_CHANGED = 43, clock_event_reaches
    ├── clocks.rs               # the rule: create/tick/show/rename/delete, clamp, gates
    ├── clocks_tests.rs
    ├── systems.rs              # clock_kinds_for_system
    └── graphql/
        ├── mutations_clocks.rs
        ├── mutations_clocks_tests.rs
        └── queries/clocks.rs   # worldClocks, worldSystemClockKinds

apps/web/src/
├── api/clocks.ts               # fetchWorldClocks, fetchClock, fetchClockKinds (postGraphQL)
├── engine/world/
│   ├── types.ts                # WorldClock, clocks in WorldState, commands and intents
│   ├── store.ts                # reducer cases
│   └── sync/
│       ├── clocks.ts           # startClockMutationBridge, applyClockWorldEvent
│       ├── index.ts
│       └── __tests__/clocks.test.ts
├── components/world/PlayDock/
│   ├── WorldDock.tsx           # DockSectionId "clocks"
│   ├── ClocksPanel.tsx
│   └── ClockRing.tsx           # the wedges; derived angle lives here
└── pages/world/useDockSections.tsx   # mount the tab when kinds exist

apps/web/e2e/
├── clocks-systems.spec.ts
├── clocks-tick.spec.ts
└── clocks-hidden.spec.ts

packs/systems/blades_in_the_dark/system.json   # clocks: progress
packs/systems/genie/system.json                # clocks: doom, puzzle
packs/systems/README.md                        # the `clocks` block
docs/guides/clocks.md                          # for Game Masters and players

scripts/e2e/slices.json                        # slice `clocks`
package.json                                   # e2e:clocks, e2e:clocks:integration
```

**Structure Decision**: the shape every synced kind already has. The rule
(`clocks.rs` in the server) is separate from the GraphQL surface
(`mutations_clocks.rs`) so the gates and the clamp are unit-tested without
a schema. No canvas-core type: the engine never reads a clock, so there is
nothing to share with wasm.

**Slice**: `clocks` is added to `scripts/e2e/slices.json` with own prefix
`clocks-`, neighbours `status-systems.spec.ts` (seam: the manifest block
validator both features share) and `genie-puzzle-clock-rewards.spec.ts`
(seam: Genie's pack-local clocks, which this feature must leave alone),
and paths covering the files above. Root `package.json` gains
`"e2e:clocks": "pnpm run e2e:clocks:integration"` and
`"e2e:clocks:integration": "node ./scripts/e2e-parallel.mjs --shards=1
--slice=clocks"`. `pnpm verify`'s slice check must pass.

## Design notes

### Reach for code 43

`roll_event_reaches` today special-cases codes 36/37. Rather than a second
special case, the subscription's filter becomes `event_reaches(event_code,
payload, is_gm_or_admin)` that dispatches by code: rolls as before, 43 on
`payload.shown`, everything else true. The rewrite of a hide into a
`deleted` for players (FR-008) is done where the event is recorded, by
recording the true action with `shown: false`; the client treats any 43
with `shown: false` it is allowed to receive as a removal unless it runs
the world. So the server never records two events for one write.

### Clamp in SQL

`UPDATE world_clocks SET filled = LEAST(segments, GREATEST(0, filled + $by)),
updated_by = $user, updated_at = now() WHERE id = $id AND filled <>
LEAST(segments, GREATEST(0, filled + $by)) RETURNING *`. Zero rows means a
no-op tick, refused with "clock is full" or "clock is empty" chosen from
the sign of `by`. Two concurrent +1 ticks both match and sum.

### Dock mounting

`useDockSections` already takes the world and its system. It reads
`worldSystemClockKinds` once per world (a plain fetch with `refetch()`, as
AGENTS.md prescribes for what the store does not hold) and mounts the
`clocks` section when the list is non-empty. The panel receives the kinds
as props and the store as the dock's other panels do.

### Why no `sceneClocks` query

The store holds every clock the client may see for the world; a scene
change is a filter in the panel, not a fetch. One query, one subscription
code, no scene-scoped resubscribe.

## Complexity Tracking

None. No new dependency, no new service, no new abstraction layer.
