# Generated Engine SDK, and the ECS/React Split for Status Presentation

- **Date**: 2026-08-29
- **Status**: Accepted
- **Spec**: `specs/029-in-engine-status-displays/`

## Context

Two decisions arose while planning spec 029 (in-engine status displays). Both
outlive that feature, which is why they are recorded here rather than left in
the plan.

### The engine's command boundary is untyped

The application drives the engine through one entry point:

```rust
#[wasm_bindgen]
pub fn apply_world_command(json_command: &str)
```

Every shape crossing it is defined in Rust and mirrored **by hand** in
TypeScript. There is nothing connecting the two, so they drift — and the
failure mode when they do is the worst available one: the engine deserializes
what it recognises and ignores the rest, so a renamed or mistyped field
produces a display that silently does not appear. No error, no warning, no
log. The symptom shows up as "the feature doesn't work" with nothing to
attach a debugger to.

This has already happened in this codebase in adjacent forms. The engine's
`Token` component carried a `token_type` field that nothing stored and nothing
drew. `WorldTokenPayload` did not deserialize `health`/`maxHealth` although
the web client had been sending both since spec 004 — the values were dropped
at the boundary and nobody noticed, because dropping is silent.

Spec 029 adds substantially more surface to that boundary.

### Where status presentation lives is ambiguous

Constitution Principle I says the ECS owns canvas simulation and React owns
chrome. Spec 029 has two surfaces — bars attached to tokens, and a panel in a
screen corner — and they do not obviously land on the same side.

## Decision

### 1. Types crossing the engine boundary are generated, not mirrored

Wire types are defined once in `thunderforge-canvas-core` and the TypeScript
is generated from them with `ts-rs`. The generated output is committed and a
CI check regenerates it and fails on any difference.

**They live in `thunderforge-canvas-core`, not the engine crate, and this is
forced.** Any generator must execute Rust on the host to emit output. The
engine crate builds for `wasm32-unknown-unknown`, has no
`wasm-bindgen-test` runner, and its tests compile without ever running — so
generation cannot be hosted there whatever tool is chosen. The same constraint
means any *rule* needing real test coverage belongs there too, which is why
the resource model, depletion order and quarter banding sit beside the types
rather than beside the renderer.

**`ts-rs` rather than the already-present `schemars`**, and the distinction is
by job rather than preference. JSON Schema is a validation vocabulary: a Rust
enum round-trips through it as `oneOf` and emerges as a union that narrows
poorly. The property being bought here is a discriminated union that makes a
wrong payload a compile error, and `ts-rs` emits that directly. `schemars`
keeps the work where schema-as-contract is the point — manifest validation,
published API schemas under `docs/api/schemas/`.

Commands carry an integer `sdkVersion`. A mismatch is rejected outright with a
reported error and applies nothing; silent discard is what this replaces.

### 2. Status presentation splits on spatial versus screen-space

- **Token-attached bars are ECS.** They track a token's position, scale with
  the camera, and reorder with other entities. A Bevy plugin draws them.
- **The corner panel is React.** It is screen-space, and Principle I names
  panels as legitimate React presentation.

What keeps this from becoming the two competing stores Principle I was written
against: **the engine owns the resolved display state and React observes it.**
React computes no value, holds no resource state, and makes no disclosure
decision. It reads the same resolved state the engine draws from, through the
SDK's read surface.

## Consequences

### Good

- A drifted field becomes a compile error rather than an invisible no-op.
- Tagged enums narrow on the TypeScript side, so handling a payload
  exhaustively is checkable.
- Rust doc comments carry into TSDoc, so warnings travel with the type — the
  note that percentage disclosure leaks more than it appears to reaches an
  application developer's editor rather than living only in a spec.
- The corner panel keeps screen readers, text selection and browser zoom,
  which a WebGL-drawn panel would have to reimplement or lose.

### Costs

- A new Rust dependency, and two type-generation tools in the tree. The ADR
  states the split so this is not later read as duplication.
- A committed generated file can fall behind its source. The CI check is what
  makes that loud; without it this decision is weaker than the status quo,
  because a generated file nobody regenerates is a hand-written file with a
  misleading header.
- Wire types live in `thunderforge-canvas-core` rather than next to the
  `wasm_bindgen` functions that receive them. The constraint is documented in
  the crate so the placement does not read as arbitrary.

### Neutral

- `Option<T>` generates as `T | null` rather than an optional field. Stricter
  than a hand-written mirror would likely have been; the contract was
  corrected to match the generator rather than the reverse.

## Alternatives considered

**Keep hand-mirrored types.** Rejected: this is the status quo whose failure
mode is silent, and spec 029 multiplies the surface it applies to.

**`schemars` + `json-schema-to-typescript`.** Genuinely attractive — already a
dependency, already an established pattern here. Rejected because it loses
discriminated-union narrowing, which is the specific property being bought.

**Draw the corner panel in the engine.** Rejected: it would mean implementing
text layout, focus handling and accessibility inside WebGL, losing screen
readers and browser zoom, in order to obey a principle that explicitly permits
the React version.

**Let React compute display state from raw values.** Rejected: that is exactly
the second source of truth Principle I exists to prevent, and it would put
disclosure decisions on the client, where they cannot be enforced.
