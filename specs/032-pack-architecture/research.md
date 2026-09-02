# Phase 0 Research: Interfaces Shaped By Their System

Nine decisions. §1–§7 survive from the first pass, three of them amended;
§8–§11 are new and come out of the clarification session. Every finding was
checked against the code on 2026-09-02.

---

## 1. A pack is data, not a module *(unchanged, and now doing more work)*

**Decision**: An interface pack is a manifest of values — colour tokens, an
engine appearance override, and a layout declaration. It contributes no
JavaScript, no stylesheet, no module, and nothing that executes.

**Rationale**: unchanged from the first pass, but the stake has risen. When the
pack was a palette, "no code" was easy because there was nothing to compute.
Now the pack describes a character sheet, and the temptation to add one
conditional is real and will arrive attached to a genuine problem. The answer
is the same and has to hold harder: the format has nowhere to put code, and
`deny_unknown_fields` means a key that is not in the contract is a rejection
rather than an ignored value.

**The line, stated so it can be applied under pressure** (FR-003a): declaring
*where a value appears* is presentation. Declaring *what a value is* is
behaviour. "Show `strengthMod` next to `strength`" is layout. "Show
`(strength - 10) / 2`" is a computation, and belongs to the system.

---

## 2. The theme vocabulary already exists, and there is only one of it *(unchanged)*

The custom properties in `apps/web/src/styles/globals.css` under `:root` and
`.dark` are the whole runtime-swappable vocabulary.
`apps/web/src/styles/tokens.scss` looked like a second, build-time-only token
system that would have made half the app unthemeable — it is imported by
**zero** files. Applying a pack is writing custom properties onto
`document.documentElement`.

---

## 3. Bundled packs only, read from disk *(unchanged)*

No `interface_packs` table and no upload flow. Discovery is a directory
listing, which also gives FR-007 for free: Forge is present because it is in
the directory, on the same footing as anything else there.

---

## 4. The engine gets the palette through a command that already exists *(unchanged)*

`set_display_appearance` is implemented in `src/engine/src/lib.rs`, owned by
`StatusDisplayPlugin`, typed in `apps/web/src/engine/sdk/commands.ts`, and has
**no caller**. This feature is its first. No engine change for the palette.

---

## 5. Light/dark stays with the reader; the pack stays with the world *(unchanged)*

A pack declares both palettes. The Game Master picks the pack; each participant
keeps their own brightness. This is the accessibility escape hatch that
survived making the look table-wide, and FR-012a's validation floor is the
other half of it.

---

## 6. The legibility floor is WCAG contrast, computed once, at validation *(unchanged)*

Rejection rather than a warning, because FR-009 leaves a reader no setting to
escape to. Computed in the validator crate so there is one implementation.
Stated explicitly against `thunderforge_canvas_core::resource_display::luma`,
which is Rec. 709 and a near neighbour that must not be confused with WCAG
relative luminance.

---

## 7. Propagation is a world event, not a poll *(unchanged)*

`EVENT_CODE_WORLD_APPEARANCE_CHANGED`, the next free code, recorded by the
mutation and re-resolved by every client on receipt. The spec 028 catch-up then
covers a client that was offline for it, at no extra cost.

---

## 8. The contract lives in canvas-core, because that is the dependency both sides already have

**Decision**: the single system contract (FR-027) is stated in
`crates/thunderforge-canvas-core`, alongside `AttributeDeclaration`,
`ResourceDefinition` and `MovementDeclaration`.

**Findings that decided it**:

- `src/engine/Cargo.toml` and `src/server/Cargo.toml` **both already depend on
  `thunderforge_canvas_core`**. It is the only crate both sides have.
- The contract is currently declared **twice**. `src/engine/src/systems/core.rs`
  declares `trait GameSystem`; `packs/systems/dnd5e/engine/src/plugin.rs`
  re-declares `trait GameSystemTrait` with the comment *"should match the one in
  `src/engine/src/systems/core.rs` / Re-defined here to avoid cross-package
  dependency"* — and has drifted from it (`&'static str` versus `String`, a
  `version()` the other lacks).
- `src/server/src/attributes.rs` records that the engine's version "has a single
  stub implementation, nothing depends on it".

The duplication has a stated cause — avoiding a cross-package dependency — and
canvas-core is exactly the dependency that removes the reason. It is also where
this codebase has twice put rules of this kind, each time giving the same
reason: its tests execute natively, and the engine crate's do not.

**Alternatives considered**:
- *A new crate for the contract alone.* Rejected: it would be a crate holding
  one trait and the declaration types it references, which already live in
  canvas-core. Two crates that must be versioned together are one crate.
- *Leave it in the engine and have packs depend on the engine.* Rejected: the
  server would then depend on the engine to read a character's values, and the
  engine cannot build for the host.

---

## 9. The contract carries declared values, never a fixed struct

**Decision**: the contract returns `identifier → value` pairs. It names no
system's concepts in its own vocabulary.

**Finding that decided it**: the engine's existing trait returns
`DerivedStats { effective_health, armor_class, initiative, proficiency_bonus }`.
That is one ruleset's character sheet compiled into a contract. It has nowhere
to put Blades in the Dark's stress, trauma and coin; nothing to say to Fate
Core, which declares **zero** abilities and eighteen skills; and no room for
Genie's Wish Points.

This codebase has made and corrected this mistake twice already. The engine
carried `TokenAbilities { strength, dexterity, constitution, intelligence,
wisdom, charisma }` and, in `attributes.rs`'s own words, "what it stored for a
Genie character was six `None`s". Resources went the same way and arrived at
`ResourceDefinition`. A third fixed struct would be a regression with two
precedents against it.

---

## 10. Derived values are computed by the pack, server-side, and never stored

**Decision**: a system pack implements the contract in its own crate; the
server resolves stored and derived values together and returns one set.
Derived values are never written to the database.

**Findings**:

- `packs/systems/*/server` are already Cargo workspace members, compiled into
  the product. Bundled packs therefore need no runtime code loading, which is
  what ADR-029 governs and has not answered. This is the distinction the whole
  increment rests on.
- The 5e pack's own `lib.rs` says its models hold "only BASE stats, never
  derived data", and that SRD reference data is "used for derived data
  calculations **on engine/web**". That intent produced
  `packs/systems/dnd5e/web/src/derived-data.ts` — 215 lines computing
  modifiers, saves and HP by hit die — which **nothing builds or loads**: the
  package has no `dist`, and `vite.config.mts` aliases only Genie's web source
  into the bundle. Meanwhile `apps/web/src/components/game-systems/dnd5e/
  CharacterSheet.tsx` computes a dexterity modifier inline at line 208.
- The server already resolves declarations server-side —
  `src/server/src/attributes.rs` and `src/server/src/status_display.rs` do
  exactly this for attributes, movement and resources.

So the stated intent has been tried and produced two implementations, one dead.
Resolving server-side puts derivation on the path that already exists, makes it
natively testable, and gives the sheet and the canvas status bars the same
numbers rather than each deriving its own.

**Not stored**, because a derived value that is also stored is two values that
can disagree, and the stored one is the one that will be stale.

---

## 11. Layout addresses declarations generically or by name

**Decision**: a layout construct targets either a declaration *set* — "every
declared attribute, in declaration order" — or a specific identifier. Forge
uses only the generic form (FR-025b); targeted packs use names and validate
against each named system's manifest (FR-026).

**Why both are needed** — the shipping manifests differ in kind, not degree:

| System | abilities | skills | resources | movement |
|---|---|---|---|---|
| dnd5e | 6 | 18 | hitPoints | 5 |
| pathfinder2e | 6 | 18 | hitPoints, focusPoints, heroPoints | 5 |
| genie | 3 | 0 | health, wishPoints | stride |
| blades_in_the_dark | 3 | 12 | stress, trauma, coin | none |
| year_zero_engine | 4 | 12 | — | — |
| cypher_system | 3 | 0 | — | — |
| fate_core | **0** | 18 | — | — |

A layout that names identifiers cannot serve all of these; a layout that only
addresses sets cannot express a nine-level spell slot grid or a six-box death
save tracker. Generic addressing is what lets Forge be the system-agnostic
default of FR-006 as a mechanism rather than a promise — it works everywhere
precisely because it names nothing.

**Scale, from the source**: the published 5e sheet carries 336 fields — 122
checkboxes, 100 spell name slots, 18 slot counters, 9 attack fields, 5 currency
denominations, 6 abilities and 6 separate modifier fields. That is the ceiling
a targeted pack has to reach eventually. It is **not** a design to copy: FR-003b
makes published sheets a source of scope and never of layout, ornament, or
wording.

---

## 12. Discovery, not a list — mechanism undecided, and flagged as such

**Decision for now**: replace the named registration functions with discovery
over the workspace's pack crates, and add `scripts/check-system-registry.mjs`
to fail the build if a central list reappears. **This is the decision most
likely to need revisiting**, and it is recorded that way rather than as settled.

**The state today**: `src/server/src/systems.rs` has `register_dnd5e_system`
and `register_genie_system`, each naming its system and wiring five validator
functions by hand, with a comment saying the remaining five systems "all follow
`register_genie_system`'s exact pattern". `src/server/Cargo.toml` has a
`[dependencies.dnd5e_server]` and `[dependencies.genie_server]` block. Adding a
system means editing both — SC-004's violation, twice.

**The precedent is weaker than the spec assumes.** The spec's Assumptions name
the interaction seam as the pattern to follow, but `src/server/src/interaction.rs`
still assembles from a `contributions()` function. What
`scripts/check-interaction-seam.mjs` actually enforces is that the *core* owns
no effect — a different and more modest claim than "no central list".

**Alternatives**:
- *A distributed-slice crate (`inventory`, `linkme`).* True discovery, no list
  at all. Costs a dependency and behaves differently across linkers; wasm is
  the case to check before committing.
- *A generated list, plus the checker.* Honest, boring, and the checker is what
  actually holds the property. Weaker than FR-029's wording.
- *Keep the hand-written list.* Rejected: it is the thing SC-004 measures.
