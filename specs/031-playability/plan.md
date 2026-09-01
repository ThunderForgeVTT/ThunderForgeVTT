# Implementation Plan: Playability 001 — From Demonstrable to Playable

**Branch**: `031-playability` | **Date**: 2026-09-01 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/031-playability/spec.md`

## Summary

Close the seams between subsystems that already work, so a Game Master can run
a session rather than demonstrate one. The spec's 43 requirements divide along
a line the constitution already draws: canvas interaction (Place, selection
filtering, snapping, wall primitives, right-click) belongs in the **engine**;
panels, editors and lists belong in **React chrome**; and every mutation that
persists — pickup, placement, binding, imagery — is enforced **server-side**.

The technical approach is deliberately conservative: no new subsystems. Every
requirement maps onto machinery that exists — the interaction effect
contribution seam (ADR-054), the image transcode/storage path used by lore
images, the system hooks contract, the token ownership rules, the world cache.
Three items need genuinely new modelling (actor imagery, item price, lore
organisation) and one needs an architectural decision recorded as an ADR before
implementation diverges (how a token survives a scene change).

## Technical Context

**Language/Version**: Rust (edition 2024) for engine and server; TypeScript
6.0.3 for the web app.

**Primary Dependencies**: Bevy 0.19.1 compiled to WASM (canvas simulation),
**adding the `bevy_state` feature — available in 0.19.1 and currently not
enabled** (see research R11);
Axum + async-graphql + Diesel (server); React 19.2.5 + Radix + Vite 8 (chrome);
`lucide-react` 1.33 (already present, supplies the lore book icon).

**Storage**: PostgreSQL 18 (Diesel migrations under `src/server/migrations/`,
paired up/down); RustFS S3-compatible object storage for images; OPFS +
IndexedDB + WebCrypto on the client for the world cache.

**Testing**: `cargo test` for native crates (`thunderforge-canvas-core` is
where engine-adjacent rules are testable — the engine crate's own `#[cfg(test)]`
modules compile but never execute under wasm32); `vitest` for web units;
Playwright for e2e, now sharded via `scripts/e2e-parallel.mjs`.

**Target Platform**: `wasm32-unknown-unknown` for the engine; Linux server;
desktop browsers. **Browser support is currently unstated and is a live
question** — see FR-042 and research.md R7.

**Project Type**: Web application with a WASM simulation engine and
system/interface content packs.

**Performance Goals**: 60fps canvas at a real battle map; the existing
`engine-limits` gate is `fps > 20` across a token sweep, and SC-001 (ten
placements in a minute) must not regress it.

**Constraints**: Placement, selection and snapping run inside the engine's
frame budget. Pickup and scene change are multi-write operations that must not
half-apply. The world cache must not silently no-op on an unsupported browser.

**Scale/Scope**: 43 functional requirements across 9 user stories, touching the
engine, the server, and roughly a dozen web surfaces. Explicitly excludes the
six architectural themes listed in the spec's Out of Scope.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

### I. ECS Owns Simulation, React Owns Chrome — **PASS, with a hard line**

The single largest risk in this feature is implementing Place as a React ghost
element following the cursor. That would make React a second source of truth
for canvas state and would not survive contact with the camera or the grid.

- **Engine (Bevy plugins)**: cursor-attached placement and its cancel (FR-004,
  FR-005), selection filtering (FR-008), grid snapping including hex (FR-024,
  FR-025), wall room/door primitives (FR-026), canvas right-click (FR-029), and
  the scene load/unload transition (FR-018).
- **React chrome**: the Select filter menu itself, the in-pane character view,
  panels, editors, lists, admin navigation, the loading indicator (FR-041).
- The in-pane character view (FR-002, FR-003) observes engine/world-store state
  for presentation; it must not recompute anything the engine or a game system
  already resolves.

### II. Plugin-Modular Engine Architecture — **PASS**

New engine capability ships as self-contained plugins under
`src/engine/src/plugins/`, addable and removable independently, communicating
by events. Placement, selection filtering and wall primitives are separate
concerns and must not reach into each other. Door primitives are contributed
from the existing wall module, consistent with ADR-054.

**The mode machinery is Bevy's, not ours.** Three of this feature's behaviours
are state machines, and `bevy_state` already expresses them idiomatically —
transitions, and `OnEnter`/`OnExit` hooks that run exactly once per transition.
Building a bespoke mode flag, or letting React chrome hold the active mode,
would put engine state outside the engine and violate Principle I. See the
section below.

### III. Ownership & Authorization at the Data Boundary — **PASS**

Every persisted mutation here is server-enforced, with the client permitted to
apply optimistically: item pickup (FR-015 through FR-017), token placement
(FR-007), player-character binding (FR-034), actor imagery (FR-036), item price
(FR-037), lore organisation (FR-038). New tables carry `created_by`/`updated_by`
per convention. Concurrent pickup (FR-016) is the same race spec 017 already
settles for character claims and must be resolved at the database boundary, not
in the client.

### IV. Real ADRs and Specs Before Divergent Implementation — **CONDITIONAL PASS**

Three decisions in this feature are architecturally significant and must land
as ADRs in the same change set, not retroactively:

1. **Token survival across a scene change** (FR-019). ADR-040 unified the token
   backing store onto the scene-scoped `tokens` table; "bring the party" either
   re-creates tokens in the new scene or introduces tokens that follow a party.
   That is a change to an ownership boundary and needs an ADR.
2. **Actor imagery model** (FR-036). Two scalar columns versus rows keyed by
   role. The deferred VTuber set (talking/not-talking/background) is *n* images,
   so the cheap choice now forecloses the later one.
3. **Item price placement** (FR-037). A generic price alongside
   `world_genie_shop_listings` risks a second economy; the relationship between
   presentational price and system-owned economy must be recorded.

### V. Verify Before Claiming Done — **PASS**

Per-crate checks: `cargo check --target wasm32-unknown-unknown` for the engine,
native `cargo check` for the server, `tsc`/build for the web app, and — for
every UI-affecting requirement here — exercised in a running dev instance. This
feature originates from a playtest; several of its defects were invisible to
the automated suite, so hand-verification is not optional.

### DMCA / Content Moderation Guardrail — **PASS, non-applicable**

Nothing in this feature makes one world's content visible, copyable or
searchable outside that world. Lore organisation (FR-038) is within-world; the
external Git sync that *would* engage this guardrail is deliberately Out of
Scope and specified separately (034).

### Constitution accuracy note

The constitution's Technology Constraints still name "RxDB-based client
replication". RxDB has been removed (`apps/web/package.json` contains no
reference); the world cache plus the engine/GraphQL bridge is now the sole sync
mechanism. This does not block the feature, but the constitution should be
corrected so it does not mislead a future contributor.

## Project Structure

### Documentation (this feature)

```text
specs/031-playability/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── graphql-mutations.md
│   ├── engine-events.md
│   └── interaction-effects.md
├── checklists/
│   └── requirements.md  # From /speckit-specify
└── tasks.md             # Phase 2 output (/speckit-tasks — NOT created here)
```

### Source Code (repository root)

```text
src/engine/src/                     # Bevy engine (wasm32 only)
├── plugins/
│   ├── placement.rs                # NEW: cursor-attached placement + cancel
│   ├── selection_filter.rs         # NEW: which kinds Select acts on
│   ├── wall.rs                     # EXTEND: room + door primitives
│   ├── grid.rs                     # EXTEND: snapping, square and hex
│   ├── interaction.rs              # EXTEND: item pickup dispatch
│   └── scene_transition.rs         # NEW: unload/load on scene change
└── lib.rs                          # plugin registration only

crates/thunderforge-canvas-core/    # Native-testable rules
└── src/                            # snapping maths, placement validity,
                                    # scene-transition retention rules

src/server/
├── migrations/                     # actor imagery, item price, lore
│                                   # organisation, selection prefs
└── src/graphql/                    # pickup, placement, binding, imagery,
                                    # price, lore tree mutations

apps/web/src/
├── components/world/
│   ├── PlayDock/                   # ActorsPanel View/Place, CombatPanel
│   ├── GmToolRail/                 # Select filter menu, wall helpers
│   └── ...                         # in-pane character view
├── pages/world/
│   ├── players/                    # cards + search + binding
│   ├── compendium/                 # NPC/item list vs create page
│   ├── actor/                      # imagery upload, interlinking
│   ├── lore/                       # tree + tags
│   └── scenes/                     # action table, Launch vs Preload
└── pages/admin/                    # sidebar navigation

apps/web/e2e/                       # updated specs + shared fixtures
```

**Structure Decision**: The existing three-surface layout is kept unchanged —
engine crate, server crate, web app — because the constitution's first
principle is precisely a statement about which surface owns what, and every
requirement here already lands cleanly on one of them. The only structural
addition is new Bevy plugins, which Principle II requires to be separate
modules rather than extensions of existing ones. Rules that can be tested
natively (snapping maths, retention predicates) go in
`thunderforge-canvas-core`, because the engine crate's tests compile but never
run under wasm32 — the same constraint that shaped specs 029 and 030.

## Engine state machines (`bevy_state`)

Three behaviours in this feature are modes with transitions, not flags. Bevy's
state module expresses them directly, and enabling it is one feature flag on a
dependency already compiled in. Verified: `bevy_state` exists in Bevy 0.19.1,
is **absent from our feature list**, and no `States`/`NextState`/`OnEnter` usage
exists anywhere in `src/engine/src` today.

| Machine | States | Why a state machine earns its keep |
|---|---|---|
| **Placement** (FR-004, FR-005) | idle → carrying → placed \| cancelled | `OnExit(carrying)` is where "leave no trace" is guaranteed *once*, rather than at every exit path — including the spec's dropped-connection-mid-carry edge case |
| **Scene transition** (FR-018) | ready → unloading → loading → ready | `OnEnter`/`OnExit` own the unload and load of tokens, walls and lights, so no system has to ask "have we finished switching yet?" |
| **Authoring mode** (FR-040, FR-040a) | select \| walls \| lights \| shapes \| tokens \| interactions | Makes "which tool is active" one authority with explicit transitions, which is exactly what FR-040a requires |

**Authoring mode is the one that pays for the others.** Today the active tool
lives in React chrome and the engine acts on ambient input. Research R6 records
the stray-marker defect and the detail that it misfires for every tool *except*
text — the one tool handled in the DOM rather than the engine. A mode owned by
the engine, changed only by an explicit transition, is a structural answer to
that class rather than a patch to one symptom.

**Scope discipline**: `bevy_state` is for *engine modes*. It has no bearing on
offline sync (already solved by spec 028), session replay, or telemetry — those
are server-side questions and explicitly not part of this feature.

## Phasing and cross-spec dependencies

The spec's priorities are user value; delivery order must also respect what
blocks what.

| Order | Content | Rationale |
|-------|---------|-----------|
| 0 | Enable `bevy_state`; introduce the authoring-mode machine | Prerequisite for FR-040a and the structural fix for FR-040; the placement and scene-transition machines build on the same feature |
| 1 | US9 defects (FR-040 to FR-043) | Small, independent, and two of them erode trust in everything else. FR-042 needs the browser-support decision first (R7). |
| 2 | US1 + US2 (View/Place, selection) | The session itself. Highest value, and the in-pane view can use the existing sheet registry as-is. |
| 3 | US3 interaction primitives | Depends on nothing above but is heavier: `lore.open` exists, `item.pickup` is new and must be contributed by the item subsystem per ADR-054. |
| 4 | US5 Launch/Preload, US4 scene lifecycle | US4 is gated on the ADR named in Constitution Check IV.1. |
| 5 | US6 authoring helpers, US8 content management | Preparation-time value; US8 carries the e2e churn noted in the spec's Assumptions. |
| 6 | US7 combat | **Blocked on spec 032.** FR-031 requires turn structure supplied by the game system, which requires packs to contribute surfaces — the static `SYSTEM_ACTOR_SHEETS` registry cannot express it. FR-030 (selected tokens feed the roster) is independent and may ship earlier. |

**Cross-spec dependency**: FR-031 and, in the general case, FR-002's
"the sheet the active game system supplies" both lean on 032's system-pack
contribution. For genie the existing registry suffices, so US2 ships now; US7
should not be started until 032 is planned.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Three new engine plugins rather than extending existing ones | Principle II requires independently addable/removable plugins; placement, selection filtering and scene transition are unrelated concerns with their own state | Folding placement into the token plugin is what forced a rewrite to extract selection previously — the constitution cites that history directly |
| Rules duplicated into `thunderforge-canvas-core` rather than tested in the engine crate | The engine crate targets wasm32 and its `#[cfg(test)]` modules never execute; snapping and retention rules are exactly the kind of logic that must be tested | Testing them only through Playwright makes a maths bug cost a browser run to find — the failure mode this session spent hours on |
