# ADR-044: Dice Rolling Engine — Shared Crate and Server-Authoritative Trust Boundary

**Date:** 2026-08-23
**Status:** ACCEPTED
**Participants:** ThunderForgeVTT Team
**Extends:** ADR-038 (Canvas-Core Crate Split for Native Testability)
**Supersedes:** `packs/systems/dnd5e/engine/src/dice.rs` (see Consequences)

---

## Problem Statement

Spec 014 asked for a general-purpose dice-formula engine (`NdM`,
keep/drop, reroll/exploding, success-counting, fate dice, nested
sub-expressions, named placeholders) importable by both the native
server and the `wasm32-unknown-unknown` engine, where the *server* is
the only party ever trusted to produce an authoritative result — the
client only ever names a formula and, later, watches an animation
reveal an already-decided outcome.

That trust boundary is a materially different, more security-sensitive
shape than ADR-038's canvas-core split (which only needed
target-portability, not a client/server authority split). It also
directly replaces an existing, concrete, live instance of the problem
this spec exists to solve: `packs/systems/dnd5e/engine/src/dice.rs`
(149 lines) implements a narrow `roll_d20`/`RollAdvantage` API,
`#[cfg(target_arch = "wasm32")]`-gated to a **non-functional stub that
always returns a face value of 10** on the browser build, with an
explicit `// Phase 4.8.1 will add crypto-based randomization for
browser` TODO that was never followed up. Per Constitution Principle IV
("real ADRs and specs before divergent implementation... replacing an
established dependency"), that supersession needs to be recorded
somewhere a future contributor touching the dnd5e pack will actually
find it — not just in this feature's own `specs/014-.../research.md`,
which is scoped to this feature's implementers.

## Decision

**1. Shared crate, no target-awareness inside it.** `crates/thunderforge-dice`
(package `thunderforge_dice`) follows `crates/thunderforge-canvas-core`'s
established pattern exactly: a `[workspace]` member with zero
`bevy`/`wasm-bindgen` dependency, consumed as a plain path dependency by
both `src/server` (native) and `src/engine` (`wasm32-unknown-unknown`).
Its only dependencies are `serde`/`serde_json` (wire format) and
`rand_core` (the trait-only half of the `rand` ecosystem — no OS/thread
entropy facility, and it compiles identically on every target).

**2. The evaluator never owns entropy.** `thunderforge_dice::resolve(formula,
bindings, rng: &mut impl rand_core::Rng)` takes the RNG as an injected
parameter. It is never constructed, seeded, or reached for inside the
crate. `src/server`'s `rollDice` mutation is the **only** call site in
the entire system that constructs a real, OS-entropy-seeded RNG
(`StdRng::from_rng(&mut rand::rng())`) and treats the result as
authoritative — it verifies world membership, resolves, and persists a
`world_roll_records` row only on success. `RollDiceInput` (the GraphQL
input) has no field that could express a pre-computed result: a
client-supplied outcome is structurally impossible to submit, not just
policy-rejected. `src/engine` depends on the same crate only for
formula parsing/validation (`validateDiceFormula`-style UX) and to
animate a reveal from a response's already-known per-die detail — it is
never wired to anything that treats a client-side `resolve()` call as
final.

**3. Trust lives at the API/application boundary, not via
capability-hiding inside the crate.** This is the direct fix for
`dice.rs`'s failure mode: that file couples "am I on wasm" to "do I have
real randomness," via a `#[cfg(target_arch = "wasm32")]` branch that
quietly rotted into a hardcoded stub. `thunderforge_dice` has no such
branch at all — every target gets the identical evaluator, and trust is
enforced entirely by *who calls it with a real RNG and persists the
result*, which is a single, auditable choke point (`mutations_roll.rs`)
rather than a per-target code path that can silently diverge again.

**4. Bounded iteration is enforced inside the evaluator, not left to
caller discipline.** A formula like `1d6x>=1` (explodes on any roll,
including its own explosions) has no natural termination. Two hard
`pub const` caps — total dice per resolution, reroll/explosion
iterations per individual die — are checked continuously during
evaluation and abort with a specific `FormulaError` rather than hanging
or truncating silently. This lives in the crate itself so `resolve()` is
safe to call correctly by construction, not something every caller must
remember to wrap in a timeout.

## Alternatives Considered

- **Keep dice logic ruleset-specific, duplicated per pack** (the status
  quo `dice.rs` shape): rejected — this is precisely the "two
  competing implementations" failure mode Constitution Principle I's
  rationale warns about, already manifesting as a silently-broken
  browser build.
- **`#[cfg(target_arch = "wasm32")]`-gate real randomness inside the
  crate** (mirror `dice.rs`'s own pattern, just done more carefully):
  rejected — this is the exact pattern being replaced; it couples the
  crate to target-detection instead of an explicit, testable API
  contract and makes deterministic unit testing target-conditional
  instead of "pass in a mock `Rng`."
- **A `nom`/`pest`/`chumsky` parser-combinator dependency**: rejected for
  the grammar parser itself — the grammar (spec.md FR-004–FR-009a) is
  fully enumerable and small enough that hand-rolled recursive descent
  is not meaningfully more code, and keeps a security-relevant parser's
  every branch explicit and auditable rather than routed through
  combinator-library internals.
- **Wall-clock timeout around `resolve()`, no in-crate dice cap**:
  rejected as the sole safety mechanism — a timeout still burns
  CPU/memory up to the deadline and requires every caller (server *and*
  engine) to independently remember to apply it.

## Rationale (Y-Statement)

In the context of building a dice-rolling engine that must be
importable by both a native server and a WASM game engine while
remaining impossible for the client to spoof, facing an existing,
already-broken precedent (`dice.rs`) that coupled trust to
target-detection, we decided to put the crate's only capability behind
an injected-RNG function signature and enforce authority entirely at
the GraphQL mutation boundary, to achieve a single auditable trust
choke point and a crate that is identically testable and correct on
every target, since target-conditional trust logic has already proven,
concretely, in this codebase, to silently rot.

## Consequences

- **Positive**: `thunderforge_dice` is fully unit-testable with zero
  external dependencies (`cargo test -p thunderforge_dice`, 13 tests,
  deterministic via a scripted mock `Rng` — no OS entropy, no DB, no
  wasm toolchain needed) and compiles identically to native and
  `wasm32-unknown-unknown` with no internal branching.
- **Positive**: a client-supplied roll result is not merely rejected by
  policy — there is no field in the GraphQL schema that could express
  one, and no code path that would treat a client-computed
  `resolve()` call as authoritative even if one existed.
- **Negative**: the crate's dice-notation grammar is this repo's own
  concrete syntax choice (documented in
  `crates/thunderforge-dice/src/parser.rs`'s module doc comment) rather
  than an exact clone of any single existing tool's notation — a
  deliberate, spec-sanctioned choice (spec.md's Assumptions target "the
  general family of advanced tabletop notation," not one product's
  syntax), but means any future formula-authoring UI should link to
  this crate's own grammar reference rather than assuming, e.g., Roll20
  syntax works verbatim.
- **Follow-up (explicitly out of scope for spec 014's own tasks, not
  silently absorbed)**: `packs/systems/dnd5e/engine/src/dice.rs`'s
  `roll_d20`/`RollAdvantage` call sites should be migrated onto
  `thunderforge_dice::resolve()` with equivalent formulas (`1d20` for a
  plain check, `2d20kh1`/`2d20kl1` for advantage/disadvantage — both
  directly expressible via this crate's keep-highest/keep-lowest
  modifiers with no special-casing needed), after which `dice.rs`
  should be deleted rather than kept as a second, now-redundant
  implementation.
