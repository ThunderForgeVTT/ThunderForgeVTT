# Implementation Plan: In-Engine Status Displays and the Engine UI SDK

**Branch**: `029-in-engine-status-displays` | **Date**: 2026-08-29 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/029-in-engine-status-displays/spec.md`

## Summary

Draw each token's game resources — health, stamina, mana, whatever the active
system declares — as bars and counters attached to the token, and present the
selected token's full set in a screen corner. Which resources exist is the
game system's declaration; the engine renders and knows nothing about
meaning. A resource is an ordered list of entries rather than a
current/maximum pair, which is what makes multi-stage boss bars and shields
expressible and makes "value above maximum" an unrepresentable state rather
than a handled one.

Disclosure is a first-class part of the feature, not a filter over it. A Game
Master sets one of four states per token per resource — visible, greyed,
percentage, chunked — and the coarsening happens server-side, so a client is
never sent a figure it may not display.

The second deliverable is the boundary itself: a typed, versioned TypeScript
SDK generated from the Rust types, replacing the current
`apply_world_command(jsonString)` surface where hand-mirrored shapes drift and
a drifted field fails silently.

## Technical Context

**Language/Version**: Rust 2024 edition (engine, server); TypeScript 5.x
(web); React 19

**Primary Dependencies**: Bevy (engine, wasm32 target), `wasm-bindgen`,
`ts-rs` for Rust→TypeScript type generation hosted in
`thunderforge-canvas-core` (resolved in [research.md §1–2](./research.md);
`schemars` stays for schema-as-contract work), async-graphql + diesel
(server), Zustand world store (web)

**Storage**: PostgreSQL. Resource values already live in
`world_actor_system_data` (five JSONB columns). Disclosure state is net-new
and needs a per-token, per-resource home.

**Testing**: `cargo test` for `thunderforge-canvas-core` (native, executes);
`cargo check --target wasm32-unknown-unknown` for the engine crate (its tests
compile but do not run — see Constitution V); vitest for web; Playwright for
end-to-end; the existing torture scenarios for the delivery path

**Target Platform**: Browser (wasm32-unknown-unknown engine + React shell),
Linux server

**Project Type**: Web application with a WASM simulation engine

**Performance Goals**: No reduction in the engine's measured interactive token
capacity (documented at 3,200 sprites at 60fps) beyond a stated, measured
figure. Status furniture is per-token draw cost and multiplies.

**Constraints**: Engine crate compiles only under wasm32. Coarsening must
happen server-side (FR-013). Appearance values must be application-supplied,
not compiled in (FR-022).

**Scale/Scope**: Up to ~100 tokens visible in a scene in ordinary play;
capacity sweep runs to thousands. Four disclosure states × N resources ×
tokens on screen.

## Constitution Check

_GATE: Must pass before Phase 0 research. Re-check after Phase 1 design._

### I. ECS Owns Simulation, React Owns Chrome — **PASSES, and decides the split**

This principle is the sharpest constraint on the feature and resolves an
ambiguity in the spec. The two surfaces land on opposite sides of it:

- **Token-attached bars are ECS.** They are spatial: they track a token's
  position, scale with the camera, occlude and reorder with other entities.
  They ship as a Bevy plugin and are drawn by the engine.
- **The corner panel is React chrome.** It is screen-space, not spatial, and
  the principle explicitly names panels as legitimate React presentation.

What keeps this from becoming the "two competing stores" the principle was
written against: **the engine owns the resolved display state, and React
observes it.** React does not compute what a bar should show, does not hold
resource values, and does not decide disclosure. It reads the same resolved
state the engine draws from, through the SDK's read surface (FR-021, which
exists for testing and serves this too).

Building the corner panel in the engine instead would mean implementing text
layout, focus and accessibility inside WebGL — losing screen readers,
selection and browser zoom, to obey a principle that explicitly permits
panels in React.

### II. Plugin-Modular Engine Architecture — **PASSES**

Status displays ship as a self-contained plugin under
`src/engine/src/plugins/status_display.rs`, addable and removable from the
`App` builder without editing another plugin's internals. It communicates
through shared resources and events, not direct calls into `TokenPlugin`.

One dependency needs care: the plugin must attach to token entities that
`TokenPlugin` spawns. That is an observation of shared components, not a call
into private systems, and is the same relationship the selection plugin
already has.

### III. Ownership & Authorization at the Data Boundary — **PASSES, with work**

Disclosure state is a persisted per-token setting that changes what other
people can see, so it is squarely a mutation requiring server-side
authorization. It must:

- be gated on the world role through `thunderforge_authz`
  (`runs_the_world()`), not a new parallel check;
- carry `created_by`/`updated_by` provenance on any new table, per convention;
- perform coarsening server-side so the client is never trusted with a figure
  it may not display.

The engine and client may render optimistically but the server remains
authoritative — including for the disclosure decision itself.

### IV. Real ADRs and Specs Before Divergent Implementation — **NEEDS AN ADR**

The spec exists. This feature also introduces two architecturally significant
decisions that belong in an ADR landing in the same change set:

1. **A generated, versioned TypeScript SDK** replacing the untyped
   `apply_world_command` boundary — a change to how every future engine
   capability is addressed, not just this one.
2. **The ECS/React split for status presentation** recorded above, so the
   next person adding an in-engine surface does not have to re-derive it.

### V. Verify Before Claiming Done — **PASSES**

Per-crate targets are known and already used in this repository:
`cargo check --target wasm32-unknown-unknown` for the engine,
native `cargo test` for `thunderforge-canvas-core` and the server, `tsc` and
vitest for web. The engine crate's own tests compile but never run, so any
logic that must actually be _executed_ by a test belongs in
`thunderforge-canvas-core`, which is why the resource model goes there rather
than into the engine crate.

## Project Structure

### Documentation (this feature)

```text
specs/029-in-engine-status-displays/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
└── tasks.md             # Phase 2 (/speckit-tasks — not created here)
```

### Source Code (repository root)

```text
crates/thunderforge-canvas-core/src/
└── resource_display.rs        # Entry model, depletion order, chunking,
                               # disclosure application. Pure; tests execute.

src/engine/src/
├── plugins/status_display.rs  # The Bevy plugin: bars/counters attached to tokens
├── components.rs              # TokenStatus component; Token finally attached
├── derived_data.rs            # Existing, currently never executing — this
                               # feature is its first real consumer
└── lib.rs                     # SDK entry points; type export for generation

src/server/
├── migrations/                # token_resource_disclosure (new)
├── src/graphql/
│   ├── mutations_tokens.rs    # setTokenDisclosure
│   └── queries/               # resolved, coarsened status per viewer
└── src/auth/                  # reuses thunderforge_authz; no new rules

packs/systems/*/               # ResourceDefinition declarations per system
└── server/                    # manifest extension

apps/web/src/
├── engine/sdk/                # Generated types + typed command wrappers
├── components/StatusPanel/    # The corner panel (React chrome)
└── engine/world/sync/         # Status changes over the existing event path

docs/adrs/                     # ADR for the SDK boundary and the ECS/React split
```

**Structure Decision**: The feature spans all four existing layers rather than
introducing a new one. The one deliberate placement is
`resource_display.rs` in `thunderforge-canvas-core` rather than the engine
crate: the engine's tests compile but never execute, so any rule that needs
real test coverage — depletion order, quarter banding, disclosure application
— must live in the crate whose tests run. That is the same reasoning that put
the cursor, relay and token-kind logic there.

## Constitution Re-Check (post-design)

_Required after Phase 1. Re-evaluated against the artifacts as designed._

| Principle                                 | Verdict                                 | What the design does about it                                                                                                                                                                                                                                                                                                     |
| ----------------------------------------- | --------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| I. ECS owns simulation, React owns chrome | **PASSES**                              | Confirmed by the contracts: the engine receives _resolved_ status and draws token-attached bars; the React panel reads the same state through `getTokenStatus`. React never computes a value, holds resource state, or decides disclosure — so there is no second source of truth, which is what the principle exists to prevent. |
| II. Plugin-modular engine                 | **PASSES**                              | One plugin, `plugins/status_display.rs`, addable and removable from the `App` builder. It observes components `TokenPlugin` owns rather than calling into it — the same relationship selection already has.                                                                                                                       |
| III. Authorization at the data boundary   | **PASSES, strengthened**                | `setTokenDisclosure` requires `runs_the_world()` through `thunderforge_authz`; `token_resource_disclosure` carries `created_by`/`updated_by`. The design goes further than the principle requires: resolution is server-side, so a client is never _sent_ what it may not show.                                                   |
| IV. ADR before divergent implementation   | **OUTSTANDING — blocks implementation** | Two decisions need an ADR landing in the same change set: the generated versioned SDK replacing `apply_world_command`, and the ECS/React split recorded above. Both outlive this feature.                                                                                                                                         |
| V. Verify before claiming done            | **PASSES**                              | `quickstart.md` names the correct target per crate and states plainly that a native check on the engine crate is not a signal and that its `#[cfg(test)]` modules never execute.                                                                                                                                                  |

**One gate remains open**: the ADR under Principle IV. It is not a research
question — both decisions are made and written up here — so it is a drafting
task for the implementation phase rather than a reason to re-open Phase 0.

**Design changes made because of this re-check**: none. The Phase 0 decision
to host rules in `thunderforge-canvas-core` rather than the engine crate was
itself driven by Principle V, so the gates were already shaping the design
before this pass.

## Complexity Tracking

No constitution violations require justification. The two items below are
recorded because they are places where the plan deliberately does more work
than the smallest possible version, and a reviewer should see why.

| Decision                                              | Why Needed                                                                                                                                                   | Simpler Alternative Rejected Because                                                                                 |
| ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------- |
| Generated TS types rather than hand-written mirrors   | Every shape crossing `apply_world_command` today is hand-mirrored and drifts; a drifted field fails silently because the engine ignores what it cannot parse | Hand-mirroring this feature's substantially larger surface multiplies an existing, already-observed failure mode     |
| Resource model in `canvas-core`, not the engine crate | The engine crate's tests compile but never run, so rules placed there are effectively untested                                                               | Placing it in the engine keeps it beside the renderer but makes depletion order, banding and disclosure unverifiable |
