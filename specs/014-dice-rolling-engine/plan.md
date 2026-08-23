# Implementation Plan: Dice Rolling Engine

**Branch**: `014-dice-rolling-engine` | **Date**: 2026-08-23 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/014-dice-rolling-engine/spec.md`

## Summary

Add a new standalone, zero-Bevy/zero-wasm-bindgen-dependency crate (`crates/thunderforge-dice`, following the exact precedent already set by `crates/thunderforge-canvas-core`) implementing a generic dice-formula grammar (base `NdM`/`dF` dice, arithmetic, keep/drop, reroll/exploding, clamping, success/failure counting, parenthetical/pool composition, named-placeholder substitution) and a pure, RNG-agnostic `resolve(formula, bindings, rng) -> RollResolution` evaluator. `src/server` depends on it as a path dependency and is the only caller that ever supplies a real (OS-entropy-backed) RNG and persists the result as an authoritative `world_roll_records` row; a new GraphQL mutation (`rollDice`) is the sole way to trigger a roll and always re-resolves server-side regardless of anything the client sends. `src/engine` (WASM) also depends on the crate — for formula parsing/validation only, and for driving the dice-bouncing animation off the per-die detail the server's mutation response returns — but never calls `resolve()` with a real entropy source to produce anything treated as authoritative, since nothing downstream ever accepts a client-originated result. This plan also flags and documents (research.md §5) that this crate immediately supersedes `packs/systems/dnd5e/engine/src/dice.rs`, an existing ad-hoc, ruleset-specific, WASM-stubbed (`// Phase 4.8.1 will add crypto-based randomization for browser`, currently hardcoded to always return 10) dice roller that predates this spec and is exactly the kind of divergent implementation this feature is meant to replace — migrating dnd5e's own `roll_d20` call sites onto the new crate is out of scope for this spec's own tasks but is called out as required follow-up work.

## Technical Context

**Language/Version**: Rust 2024 edition, workspace-wide (`crates/thunderforge-dice` new; consumed by `src/server` native and `src/engine` wasm32-unknown-unknown)

**Primary Dependencies**: New crate has minimal dependencies, matching `thunderforge-canvas-core`'s established "deliberately no bevy/wasm-bindgen dependency" philosophy (Constitution Principle II) — `serde`/`serde_json` (already workspace-standard, for `RollResolution`/`DiceFormula` (de)serialization across the GraphQL boundary) and `rand_core` (the trait-only, `no_std`-friendly half of the `rand` ecosystem, so the crate accepts any `RngCore` implementation without depending on a concrete RNG or any OS/entropy facility itself — see research.md §3). No parser-combinator crate (`nom`/`pest`/`chumsky`) is added; the grammar is hand-rolled recursive-descent (research.md §2) to keep the crate's dependency footprint at the same minimal level as `thunderforge-canvas-core`'s. `src/server` adds `rand` (OS-backed `ThreadRng`/`StdRng` seeded from `OsRng`) as the one real entropy source, used only server-side. `src/server` also gains `async-graphql`/Diesel wiring for the new `rollDice` mutation and `world_roll_records` table (existing `axum`/`async-graphql`/Diesel stack, no new major dependency there).

**Storage**: PostgreSQL via Diesel (new table: `world_roll_records`, storing the resolved formula, full per-die JSON detail, final result, triggering user, world/context reference, and timestamp — FR-014). No object storage involved.

**Testing**: `cargo test` for the new crate (unit tests over the grammar/evaluator — every modifier category from spec.md User Story 2 gets a deterministic test using a seeded/mock `RngCore`, since the crate's evaluator takes an injected RNG), `cargo test` in `src/server` for the `rollDice` resolver (matching existing `#[tokio::test]` convention), `cargo check --target wasm32-unknown-unknown` for `src/engine` to confirm the new crate compiles cleanly there (Constitution Principle V), Playwright for the animation/reveal contract (`apps/web`).

**Target Platform**: Cross-compiled crate — native (Linux server) and `wasm32-unknown-unknown` (browser, via `src/engine`'s existing Bevy/wasm-bindgen pipeline). No OS-specific or browser-specific API is used inside the new crate itself, so no `#[cfg(target_arch = "wasm32")]` branching is needed within it (research.md §3) — target-specific behavior (which RNG is supplied) lives entirely in the two *callers*, not the crate.

**Project Type**: Existing Cargo workspace (`Cargo.toml` at repo root already lists `crates/thunderforge-canvas-core` and `crates/pack_system_spec` as sibling shared crates) plus the existing `src/server` + `apps/web` web-app split. This feature adds one new workspace member (`crates/thunderforge-dice`); no new top-level project or non-Cargo build unit.

**Performance Goals**: A triggered roll's server round-trip (formula parse + evaluate + persist + respond) completes fast enough that the animation (a fixed-length presentation, not something the server should be slow for) is the visible bottleneck, not the resolution itself (SC-004 speaks to "a few seconds" total including animation, so resolution itself should be sub-100ms for any formula within the FR-012 dice-count bound).

**Constraints**: FR-001/FR-012/FR-015 drive the two hardest constraints: (1) the crate itself must have zero capability to be mistaken for an authoritative source — enforced architecturally by making `resolve()` take an injected `RngCore` (the crate never reaches for its own entropy) and by the GraphQL layer never accepting a client-supplied result (contracts/graphql-roll.md); (2) a bounded-iteration guarantee on reroll/exploding/dice-count so no formula can hang or exhaust resources — enforced inside the crate's evaluator itself (a hard cap on total dice + total reroll/explosion iterations per resolution, research.md §4), not left to caller discipline.

**Scale/Scope**: Per-roll, stateless resolution (no session/queue state carried between rolls, per spec.md Edge Cases — concurrent rolls are fully independent). Roll Records are per-world, similar retention posture to spec 012's lore revisions (retained indefinitely by default, per spec.md Assumptions).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS owns simulation, React owns chrome)**: The new crate itself is pure logic with no ECS/canvas involvement — PASS. The *animation* (User Story 4) is a Bevy-side concern: dice-bouncing must be implemented as engine systems/plugins driven by the server's returned per-die detail, not as a React/DOM animation layered outside the canvas, consistent with this principle's "all canvas... tools are built as Bevy plugins/systems" rule. Flagged for research.md/data-model.md to make explicit so a later planning pass doesn't accidentally build the animation in React. PASS (with a design note carried into research.md §6).
- **Principle II (Plugin-modular engine architecture)**: Directly satisfied by design — the crate follows `thunderforge-canvas-core`'s exact established pattern (a `crates/` library with zero `bevy`/`wasm-bindgen` dependency, consumed by `src/engine` as a plain Rust dependency, independently unit-testable outside any Bevy `App`). The engine-side *consumer* of the crate (the animation trigger/playback) is expected to land as its own Bevy plugin under `src/engine/src/plugins/` when that work is scheduled, matching every other canvas capability. PASS.
- **Principle III (Ownership & authorization at the data boundary)**: The `rollDice` mutation enforces world-membership (every roll happens in the context of a world/session) server-side before resolving, and the persisted `world_roll_records` row carries `created_by`/`world_id` provenance consistent with existing convention (contracts/graphql-roll.md). This principle's spirit — "server is authoritative, client is never trusted" — is this entire feature's FR-001, so it's satisfied at the architectural level, not just the data-boundary level. PASS.
- **Principle IV (Real ADRs and specs before divergent implementation)**: This feature already has a Spec Kit spec (specs/014-dice-rolling-engine/spec.md) and this plan, satisfying the spec half of this principle. On the ADR half: **an ADR is warranted and recommended before/alongside implementation**, for two reasons found during this planning pass (research.md §5): (a) this is a new shared-crate architectural pattern crossing both native and WASM targets with a hard client/server trust boundary — a materially different, more security-sensitive shape than `thunderforge-canvas-core`'s existing "no bevy dependency" precedent, worth its own recorded rationale; (b) this feature **supersedes an existing divergent implementation** — `packs/systems/dnd5e/engine/src/dice.rs`'s ad-hoc, ruleset-specific, currently-non-functional-on-WASM dice roller — which is precisely the "replacing an established dependency... before implementation diverges across multiple files" scenario this principle exists to catch. This plan does not block on the ADR being filed first (implementation may proceed in parallel with drafting, per the constitution's own text), but the ADR should land in the same change set as the implementation, not be skipped. CONDITIONAL PASS — flagged, not blocking.
- **Principle V (Verify before claiming done)**: Implementation phase will run `cargo test` for the new crate (native), `cargo check --target wasm32-unknown-unknown` for `src/engine` (this crate specifically must be verified against the wasm target, since that's the entire point of it existing), `cargo check`/`cargo test` (native, server crate) for the `rollDice` resolver, and a live dev-server pass triggering a roll and observing the animation, before any task is marked complete. PASS (process commitment, verified at implementation time).

**Complexity Tracking entry required** — see below; this is the one deviation from a clean pass (the ADR flag), not a violation, but recorded for visibility per the gate's own instruction to fill the table on any Constitution Check item that isn't a flat, unconditional PASS.

## Project Structure

### Documentation (this feature)

```text
specs/014-dice-rolling-engine/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   └── graphql-roll.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
crates/thunderforge-dice/              # NEW workspace member
├── Cargo.toml                         # name = "thunderforge_dice"; deps: serde, serde_json, rand_core only
└── src/
    ├── lib.rs                         # public API: DiceFormula, RollResolution, DieOutcome, resolve()
    ├── parser.rs                      # hand-rolled recursive-descent tokenizer + parser → AST (research.md §2)
    ├── ast.rs                         # formula AST types (Term, Modifier, Condition, etc.)
    ├── eval.rs                        # AST → RollResolution, given an injected `&mut dyn RngCore` and a
    │                                  #   placeholder-binding map; enforces the FR-012 iteration/dice-count cap
    └── error.rs                       # FormulaError (parse + evaluation errors, FR-011)

Cargo.toml                             # EXTENDED: add "crates/thunderforge-dice" to [workspace] members

src/server/
├── Cargo.toml                         # EXTENDED: thunderforge_dice (path dep) + rand (real RNG, server-only)
├── migrations/
│   └── <ts>_create_world_roll_records/{up,down}.sql
└── src/
    ├── schema.rs                      # extended: world_roll_records table
    ├── models.rs                      # extended: RollRecord struct
    ├── graphql/
    │   ├── types.rs                   # extended: GraphQLRollResolution, GraphQLDieOutcome, GraphQLRollRecord
    │   ├── input_types.rs             # extended: RollDiceInput (formula, bindings, context refs)
    │   └── mutations_roll.rs          # NEW — `rollDice` mutation: world-membership check, calls
    │                                  #   thunderforge_dice::resolve() with a real OS-backed RNG, persists
    │                                  #   a world_roll_records row, returns the full per-die detail
    └── (roll-history query folded into mutations_roll.rs or a small queries/roll.rs, per tasks.md)

src/engine/
├── Cargo.toml                         # EXTENDED: thunderforge_dice (path dep) — parsing/validation +
│                                       #   animation-driving use only, no real-RNG dependency added here
└── src/plugins/
    └── dice_roll/                     # NEW Bevy plugin (Principle II) — physics-ish bounce animation,
        │                              #   triggered by the server's rollDice response, rendering each
        │                              #   returned DieOutcome to its resolved face
        ├── mod.rs
        ├── systems/
        └── resources/

apps/web/src/
├── api/roll.ts                        # NEW — fetch-based GraphQL call for `rollDice`, mirrors api/items.ts style
└── types/roll.ts                      # NEW — RollResolutionRecord, DieOutcomeRecord TS types
```

**Structure Decision**: One new workspace crate (`crates/thunderforge-dice`), following the existing `crates/thunderforge-canvas-core` precedent exactly (root `Cargo.toml`'s `[workspace] members` list already establishes this as the repo's convention for shared, engine-and-server-importable logic — no new top-level directory convention is invented). `src/server` and `src/engine` both take it as an ordinary path dependency; no new service, process, or build pipeline is introduced. The Bevy-side animation is planned as a new plugin under `src/engine/src/plugins/` (Principle II), not as a `apps/web`/React component, even though the *trigger* (the "roll" button/action) is a React affordance that calls the `rollDice` GraphQL mutation — the bouncing-dice presentation itself belongs to the canvas engine.

## Complexity Tracking

> Filled per the Constitution Check's Principle IV conditional-pass, not because this plan violates any principle — it's a visibility flag, not a deviation needing rejection.

| Item | Why Needed | Simpler Alternative Rejected Because |
|------|------------|----------------------------------------|
| A new ADR is recommended (not yet filed) before/alongside implementation, covering: (a) the shared-crate-with-a-trust-boundary pattern this feature establishes, and (b) the decision to supersede `packs/systems/dnd5e/engine/src/dice.rs` | This is a security-relevant architectural boundary (client/server trust) layered on top of the already-ADR-worthy "shared crate across native+WASM" shape, and it explicitly replaces an existing implementation used elsewhere in the codebase — exactly what Principle IV asks to be recorded, not silently superseded | Skipping the ADR and only documenting the decision in research.md was considered, but research.md is scoped to *this* feature's implementers; the fact that `dice.rs` is now superseded needs to be discoverable by whoever next touches the dnd5e pack, which is what ADRs (not feature-scoped research docs) are for in this codebase |
