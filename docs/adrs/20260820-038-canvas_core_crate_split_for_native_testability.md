# ADR-038: Split Canvas-Authoring Logic into a Native-Testable Core Crate

**Date:** 2026-08-20
**Status:** ACCEPTED
**Participants:** ThunderForgeVTT Team
**Extends:** ADR-037 (Native Bevy Canvas Authoring Supersedes Wrapped tldraw)

---

## Problem Statement

`thunderforge_engine` only targets `wasm32-unknown-unknown`, and this
project has no `wasm-bindgen-test-runner`/browser configured to execute
tests compiled for that target. In practice this meant every engine-side
unit test written during this feature (wall geometry, door-state logic,
shadow-casting intersection math) only ever *compile-checked* — `cargo
check --tests` passed, but the tests themselves never ran, so logic bugs
in that code had no automated safety net at all, only type-checking.

## Decision

Extract the pure, engine-agnostic data model and algorithms for canvas
authoring (walls now; lighting and shapes as they're built) into a new
crate, `crates/thunderforge-canvas-core` (package `thunderforge_canvas_core`),
with **no dependency on Bevy or wasm-bindgen** — only `glam` (Bevy's own
math crate; `bevy::prelude::Vec2` is a re-export of `glam::Vec2`, so
values pass between the two crates with zero conversion as long as the
`glam` version is kept aligned with whatever `bevy` pulls in).

`thunderforge_engine` wraps each core type in a thin `Resource` newtype
(e.g. `resources::wall::WallSet(pub thunderforge_canvas_core::wall::WallSet)`
with `Deref`/`DerefMut`) so existing call sites (`wall_set.upsert(...)`,
field access, etc.) are unchanged, and Bevy's `ResMut` change detection
still applies transparently — it only requires the outer type to be a
`Resource`, not the inner data.

**Convention going forward**: any new canvas-authoring capability
(lighting, shapes) puts its pure data/geometry in
`crates/thunderforge-canvas-core/src/<capability>.rs`, with its own
`#[cfg(test)] mod tests`, and `thunderforge_engine`'s
`resources/<capability>.rs` stays a thin Bevy wrapper with no logic of
its own.

## Rationale (Y-Statement)

In the context of the native canvas authoring feature, facing an engine
crate whose tests can only compile-check and never execute in this
environment, we decided to extract the pure logic into a dependency-free
core crate compiled to a native target, accepting a thin wrapper-type
indirection in the engine crate, to achieve tests that actually run and
catch real bugs, since a wasm-only crate cannot exercise its own test
suite here and the alternative — leaving logic untested at runtime — is
worse than a small amount of newtype boilerplate.

## Consequences

- **Positive**: 18 wall-logic tests that previously only compile-checked
  now execute for real via `cargo test -p thunderforge_canvas_core` (0.00s,
  no wasm toolchain needed). The same pattern is available to lighting and
  shapes before more untested engine logic accumulates.
- **Positive**: matches Constitution Principle II (plugin-modular
  architecture) and Principle V (verify before claiming done) more
  literally than before — "verify" now means "run," not just "compile."
- **Negative**: one extra crate boundary and a `Deref`/`DerefMut` newtype
  per wrapped resource — minor boilerplate, justified by the payoff above.
- **Follow-up**: lighting (`resources::lighting`) and shapes
  (`resources::shape`) engine work, not yet built, should follow this
  pattern from the start rather than being extracted later.
