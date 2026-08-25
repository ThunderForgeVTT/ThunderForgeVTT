# Implementation Plan: Thunderforge Crucible Crate

**Branch**: `024-thunderforge-crucible-crate` | **Date**: 2026-08-25 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/024-thunderforge-crucible-crate/spec.md`

## Summary

Add `crates/thunderforge-crucible`: a `SessionAdjudicator` trait with two
implementations — `LocalAdjudicator` (in-process, zero-config, what every
self-hosted deployment gets today) and a network-client implementation that
delegates to a standalone `crucible-server` binary (same crate, second build
output) over HTTP. The main `thunderforge` server selects between them at
startup via `CRUCIBLE_MODE` (`local` default, `remote` + `CRUCIBLE_ENDPOINT`).
This spec builds the seam and a deliberately minimal placeholder ruleset —
not the eventual real adjudication logic (headless-shared-Bevy, per ADR-047)
and not any KEDA/orchestration layer, both explicitly future work.

## Technical Context

**Language/Version**: Rust 2024 edition (matches workspace default)

**Primary Dependencies**: `axum` (already a `src/server` dependency — reused
for `crucible-server`'s HTTP surface and the network-client's HTTP calls, no
new RPC framework per research.md §1), `serde`/`serde_json` (request/response
shapes), `reqwest` or `axum`'s own client-friendly primitives for the
network-client implementation (confirmed available transitively via existing
`reqwest` dependency in the workspace — see research.md §1), `tokio` (async
runtime, matches `thunderforge`'s existing usage)

**Storage**: N/A — this crate is stateless; it resolves a request and returns
a result, it does not persist anything. (Whatever calls it, e.g. the main
server's existing token-move mutation path, remains responsible for
persistence exactly as today.)

**Testing**: `cargo test` for the crate's own unit tests (trait
implementations, request/response (de)serialization, error paths); a small
integration test spinning up `crucible-server` in-process (via `tokio::spawn`
+ an ephemeral port) and exercising the network-client against it, proving
User Story 2's "identical result" requirement without needing a separately
running process in CI

**Target Platform**: Linux server (matches `thunderforge`'s existing
deployment target — native, not WASM; this crate has no client/browser
surface)

**Project Type**: Rust library + binary (single crate, two Cargo targets —
`[lib]` and `[[bin]]`)

**Performance Goals**: Not performance-critical in this spec's scope — the
placeholder ruleset does trivial work. Deferred to whichever future spec
implements the real ruleset (ADR-047).

**Constraints**: Zero additional configuration/ops burden for the default
(local) mode (SC-001) — this is the hard constraint on this spec, not a
performance target.

**Scale/Scope**: One new crate, ~4 new files (`lib.rs`/trait, `local.rs`,
`remote.rs`, `bin/crucible-server.rs` or equivalent), wiring into the main
server's startup config — no changes to persisted schema, no GraphQL surface
changes, no frontend changes.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS owns simulation)**: Touched directly — see
  [ADR-047](../../docs/adrs/20260825-047-crucible_session_adjudication_crate.md)
  for the full reconciliation. Resolution: this spec's placeholder ruleset is
  a pass-through that does not supersede or duplicate Bevy's simulation
  authority; the real future ruleset is planned to run the same
  plugin-modular engine code headless server-side (client-predicts,
  server-authoritative — the same pattern real-time multiplayer games use),
  which is a *better* fit for this principle than a hand-written server-side
  rules-checker, not a violation of it. That real ruleset is explicitly
  out of scope here (Assumptions, spec.md).
- **Principle II (Plugin-modular engine architecture)**: Not violated —
  this spec does not touch `src/engine` at all. Referenced only as the
  intended future direction (ADR-047), not built here.
- **Principle III (Ownership & authorization at the data boundary)**: N/A in
  this spec's scope — Crucible persists nothing and enforces no
  ownership/authorization itself; whatever main-server code eventually calls
  it remains responsible for auth exactly as today, unchanged by this spec.
- **Principle IV (Real ADRs and specs before divergent implementation)**:
  Satisfied — ADR-047 (new subsystem) plus this spec/plan, landing together,
  before implementation.
- **Principle V (Verify before claiming done)**: Applies as usual —
  `cargo check`/`cargo test` for the new crate and for `src/server` after
  wiring, both native targets (no WASM surface in this spec).

No violations requiring Complexity Tracking justification beyond what
ADR-047 already records.

## Project Structure

### Documentation (this feature)

```text
specs/024-thunderforge-crucible-crate/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md         # Phase 1 output
├── quickstart.md         # Phase 1 output
├── contracts/            # Phase 1 output
└── tasks.md              # Phase 2 output (/speckit-tasks — not created by /speckit-plan)
```

### Source Code (repository root)

```text
crates/thunderforge-crucible/
├── Cargo.toml            # [lib] + [[bin]] "crucible-server", license.workspace = true
└── src/
    ├── lib.rs             # `SessionAdjudicator` trait + request/response types
    ├── local.rs           # `LocalAdjudicator` — in-process, placeholder ruleset
    ├── remote.rs           # `RemoteAdjudicator` — HTTP client, implements the same trait
    ├── server.rs           # axum router shared by the `crucible-server` binary
    └── bin/
        └── crucible-server.rs   # thin binary entrypoint wrapping `server.rs`

src/server/
├── Cargo.toml            # add `thunderforge-crucible` as a path dependency
└── src/
    └── main.rs             # read CRUCIBLE_MODE/CRUCIBLE_ENDPOINT at startup,
                             # construct the selected SessionAdjudicator, add it
                             # to AppState (fail-fast on invalid config, FR-005)
```

**Structure Decision**: Single new workspace crate
(`crates/thunderforge-crucible`), matching the existing convention of
`crates/thunderforge-dice` and `crates/thunderforge-canvas-core` (small,
focused, independently-testable library crates depended on by `src/server`).
No frontend changes, no new top-level project — this is purely a backend
Rust addition, consistent with "Project Type: Rust library + binary" above.

## Complexity Tracking

> No unjustified violations — ADR-047 records the one principle this spec
> touches (Principle I) and its resolution. Nothing else in this table.
