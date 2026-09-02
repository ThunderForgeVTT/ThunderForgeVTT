# Contract: what a system pack implements

One contract, stated once, in `crates/thunderforge-canvas-core` — the only
crate both `src/engine` and `src/server` already depend on (research §8). Every
system pack implements it. Nothing re-declares it.

This replaces two divergent declarations: `trait GameSystem` in
`src/engine/src/systems/core.rs` and `trait GameSystemTrait` in
`packs/systems/dnd5e/engine/src/plugin.rs`, the second carrying a comment
saying it "should match" the first. Both are retired by this feature.

## Shape

```rust
/// One value a system publishes about an actor.
pub struct DeclaredValue {
    /// The system's own identifier — `strength`, `strengthMod`, `wishPoints`.
    pub id: String,
    pub value: Value,          // integer | number | text | boolean | list
    /// Whether this was read from stored data or computed from it.
    pub origin: Origin,        // Stored | Derived
}

pub trait SystemRules: Send + Sync {
    /// The system this implements, matching its manifest `id`.
    fn id(&self) -> &str;

    /// Values this system computes from stored ones.
    ///
    /// Receives everything already read from the actor's stored slots and
    /// returns only what it adds. Pure: no I/O, no clock, no randomness —
    /// the same stored values must always yield the same derived ones, or
    /// two viewers of one character see two different sheets.
    fn derive(&self, stored: &DeclaredValues) -> Vec<DeclaredValue>;

    /// The identifiers `derive` may return, declared up front.
    ///
    /// Separate from `derive` because an interface pack has to be validated
    /// against a system without running it (FR-026). A `derive` returning an
    /// identifier absent from this list is a bug in the pack, and the
    /// resolver treats it as one.
    fn derived_declarations(&self) -> Vec<AttributeDeclaration>;
}
```

**What the contract does not have**, and why it matters more than what it does:

There is no `armor_class`, no `initiative`, no `proficiency_bonus`, and no
`effective_health`. The trait being replaced had exactly those, as a fixed
`DerivedStats` struct. That is one ruleset's character sheet built into the
product — it has nowhere to put Blades in the Dark's stress and trauma, and
nothing at all to say to Fate Core, which declares zero abilities.

This codebase has made that mistake twice and corrected it twice. The engine
once carried `TokenAbilities { strength, dexterity, … }`, and `attributes.rs`
records what it stored for a Genie character: six `None`s. A third fixed struct
would be a regression with two precedents against it.

## Purity, and why it is a requirement rather than a style note

`derive` is pure. No database, no network, no clock, no randomness.

A derived value is recomputed every time an actor is read, on every client's
behalf, and is never stored (research §10) — because a derived value that is
also stored is two values that can disagree, and the stored one goes stale. If
`derive` were impure, the same character would render differently to two people
at the same table, and neither would be wrong.

This is also what makes the rule testable without a database, which is the
reason the contract lives in a natively-tested crate rather than in the engine.

## Discovery

A pack's implementation MUST be found rather than listed (FR-029). Today
`src/server/src/systems.rs` carries `register_dnd5e_system` and
`register_genie_system`, each naming its system and wiring five validators by
hand, and `src/server/Cargo.toml` names each pack crate as a dependency — so
adding a system means editing shared code in two places, which is precisely
what SC-004 measures.

The mechanism is **not settled** (research §12). Whatever is chosen,
`scripts/check-system-registry.mjs` fails the build if a hand-maintained list
of system identifiers reappears in shared server code — modelled on
`scripts/check-interaction-seam.mjs`, and for the same reason: the property is
easy to state, easy to erode, and a behavioural test cannot catch the erosion
until something has already broken.

## Resolution, end to end

1. The manifest declares what a system *has* — abilities, skills, resources,
   movement. Already true; `src/server/src/attributes.rs` and
   `status_display.rs` already parse it.
2. Stored values are read from the actor's JSONB slots. Already true.
3. `derive` adds computed values. **New.**
4. The two merge into one `identifier → value` set, `origin` distinguishing
   them, and travel to the sheet and to the canvas identically.

Step 4 is the property worth protecting: the sheet and the status bars must not
each derive their own numbers. That is the failure the two live 5e
implementations already demonstrate, one of them computing a dexterity modifier
inline in a component while the other computes it in a module nothing loads.
