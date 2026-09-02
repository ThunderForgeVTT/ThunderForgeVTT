# One System Contract, Carrying Declared Values

- **Date**: 2026-09-02
- **Status**: Accepted
- **Spec**: `specs/032-pack-architecture/` (FR-027, FR-028; `contracts/system-contract.md`)

## Context

The contract a game system implements is currently declared **twice**.

`src/engine/src/systems/core.rs` declares `pub trait GameSystem` with `id`,
`name`, `ability_names`, `skill_definitions`, `validate_token`,
`calculate_derived_stats` and `calculate_movement_cost`.
`packs/systems/dnd5e/engine/src/plugin.rs` declares `pub trait
GameSystemTrait`, above this comment, verbatim:

```rust
/// GameSystem trait - should match the one in src/engine/src/systems/core.rs
/// Re-defined here to avoid cross-package dependency
```

They do not match, and the comment is the only thing claiming they should.
The engine's returns `&'static str` from `id` and `name`; the pack's returns
`String` and adds a `version()` the engine's has never had. The engine's has
`ability_names`, `skill_definitions`, `validate_token`,
`calculate_derived_stats` and `calculate_movement_cost`; the pack's has none of
them, and instead carries `ability_modifier`, `skill_bonus`,
`proficiency_bonus` and a twenty-row `max_spell_slots` table — 5e rules, in a
trait's default methods. Two declarations kept in step by convention have
already stopped being in step, which is what FR-027 forbids.

`src/server/src/attributes.rs` records what happened to the engine's half in
the meantime:

> `src/engine/src/systems/core.rs` declares a `GameSystem` trait with an
> `ability_names() -> Vec<&'static str>`, which is the other way this could
> have gone: one compiled-in implementation per ruleset. It has a single stub
> implementation, nothing depends on it, and it duplicates a list the manifests
> already carry — every shipping system declares its own attributes in
> `system.json` today.

The stated cause of the duplication is *avoiding a cross-package dependency*.
That reason has since expired: `src/engine/Cargo.toml` and
`src/server/Cargo.toml` both already list `thunderforge_canvas_core`, and it is
the only crate both sides have.

## Decision

### 1. There is exactly one contract, and it lives in `thunderforge-canvas-core`

Every system pack implements it. Nothing re-declares it. Both `trait
GameSystem` and `trait GameSystemTrait` are retired.

`canvas-core` is where this codebase has put rules of this kind twice already —
spec 029's resource model and spec 030's effect declarations — and the reason
given each time is the same: the engine crate targets
`wasm32-unknown-unknown` with no wasm-bindgen test runner, so its
`#[cfg(test)]` modules compile and never execute. A rule placed there is
untested by construction. `canvas-core`'s tests run natively, and it is
already the crate the server compiles and from which the web app's TypeScript
is generated.

### 2. The contract carries declared values and names no system's concepts

```rust
pub struct DeclaredValue {
    pub id: String,        // the system's own identifier
    pub value: Value,      // integer | number | text | boolean | list
    pub origin: Origin,    // Stored | Derived
}
```

`SystemRules::derive(&self, stored: &DeclaredValues) -> Vec<DeclaredValue>`
receives everything read from the actor's stored slots and returns **only what
it adds**. `derived_declarations()` states the identifiers `derive` may return,
separately from `derive` itself, so an interface pack can be validated against
a system **without running it** (FR-026). A `derive` returning an identifier
absent from that list is a bug in the pack and is treated as one.

There is no `armor_class`, no `initiative`, no `proficiency_bonus`, no
`effective_health`.

## Rationale (Y-Statement)

In the context of every system pack having to supply an actor's values, facing
a contract declared twice and already drifted, we decided **one contract in
`canvas-core` carrying `identifier -> value` pairs with a `Stored`/`Derived`
origin** and neglected **the existing `DerivedStats { effective_health,
armor_class, initiative, proficiency_bonus }` struct**, to achieve **a contract
that holds every shipping system without privileging one**, accepting **that
consumers must resolve values by identifier rather than by field access**.

## The fixed struct is a mistake this repo has made twice

`DerivedStats` is one ruleset's character sheet compiled into a contract. The
eight manifests in `packs/systems/` do not agree that those four fields exist:

| System | Abilities | Skills | Resources |
|---|---|---|---|
| `fate_core` | **0** | 18 | none declared |
| `cypher_system` | 3 (might, speed, intellect) | **0** | none declared |
| `blades_in_the_dark` | 3 (insight, prowess, resolve) | 12 | stress, trauma, coin — **and no movement block at all** |
| `genie` | 3 (might, cunning, spirit) | 0 | health, wishPoints |
| `dnd5e` | 6 | 18 | hitPoints |

`armor_class` means nothing to any of the first four.

`crates/thunderforge-canvas-core/src/attributes.rs` records the first
correction:

> The engine used to carry `TokenAbilities { strength, dexterity,
> constitution, intelligence, wisdom, charisma }`, which is one game system's
> character sheet compiled into a renderer. It could hold D&D 5e and
> Pathfinder 2e. It could not hold either of the other two systems that
> already ship […] What it stored for a Genie character was six `None`s.

Resources went the same way and arrived at `ResourceDefinition` in
`crates/thunderforge-canvas-core/src/resource_display.rs`, whose header states
the general form: "The engine holds no built-in notion of 'health': one system
tracks hit points, another health/stamina/mana, a third health/energy.
Hard-coding the first would make every system after it a special case."

A third fixed struct would be a regression with two precedents against it.

## Consequences

**`derive` must be pure.** No I/O, no clock, no randomness. Derived values are
recomputed on every read and never stored, because a derived value that is also
stored is two values that can disagree and the stored one is the one that goes
stale. If `derive` were impure, two viewers of the same character at the same
table would see different sheets and neither would be wrong. Purity is also
what makes the rule testable without a database — which is the whole reason for
choosing a natively-tested crate over the engine.

**`origin` exists so a surface can tell a player which numbers they may edit.**
A 5e Strength score is typed in; its modifier is not. A text box over a
computed value invites the two to disagree, and `Stored` versus `Derived` is
what lets an interface avoid drawing one.

**Consumers address values by identifier.** No downstream code gets
`stats.armor_class`. That is the cost, and it is the one already paid for
attributes and resources.

**5e's default-method rules need a home.** `ability_modifier`, `skill_bonus`,
`proficiency_bonus` and `max_spell_slots` are 5e rules and belong in 5e's
`derive`, not in a shared trait where every other system inherits them.

## Alternatives Considered

- **A new crate holding only the trait.** Rejected: it would need
  `canvas-core`'s declaration types (`AttributeDeclaration`, and `Value`)
  anyway, and two crates that must be versioned together are one crate.
- **Leave it in the engine; packs depend on the engine.** Rejected: the server
  would then depend on the engine to read a character's values, and the engine
  does not build for the host — it targets `wasm32-unknown-unknown`.
- **Keep both declarations and test that they match.** Rejected: it tests a
  coincidence rather than removing the reason for one. The comment already
  asserts they match, and they do not.
- **Widen `DerivedStats` with optional fields per system.** Rejected: that is
  `TokenAbilities` again, and its outcome is on record — six `None`s for a
  Genie character.

## Related Decisions

- **ADR-054** — declarations live in `canvas-core` because it is the only crate
  where the rules can be tested; the same constraint applies here.
- **ADR-059** — an interface pack is data, not a module; `derived_declarations`
  is what such a pack is validated against.
