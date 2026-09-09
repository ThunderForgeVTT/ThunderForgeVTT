# ADR-074: System-Declared Checks and Sheet-Initiated Rolls

**Date:** 2026-09-07
**Status:** ACCEPTED
**Participants:** ThunderForgeVTT Team
**Extends:** ADR-044 (Dice Rolling Engine — Shared Crate and Server-Authoritative Trust Boundary)
**Related:** spec 032 (a sheet is what a system declares), spec 036 US3b

---

## Problem Statement

Spec 036 gives a player a character sheet on a second screen while the table
runs on the first, and the first thing anybody wants to do with a sheet is
roll off it. "Roll Strength" is a button. The question is who decides what
that button rolls.

ADR-044 already answers half of it: the server is the only party that may
produce an authoritative result. What it does not answer is where the
*formula* comes from. A d20 plus a Strength modifier is 5e's rule; a
stress-dice pool is Year Zero's; a 4dF ladder roll is Fate's. Putting any of
them in the sheet component puts one system's rules in shared presentation
code, which the constitution forbids and which spec 029 spent a whole feature
removing from the engine.

The manifests could not answer it either. Every pack already declares a roll
shape — but under its own key and in its own shape:

| Pack | Key | Value |
|---|---|---|
| pathfinder2e | `coreCheck` | `"1d20+modifier"` |
| blades_in_the_dark | `actionRoll` | its own |
| cypher_system | `taskResolution` | its own |
| fate_core | `ladderRoll` | its own |
| year_zero_engine | `skillRoll` | its own |
| genie | `manifestationRoll` | its own |
| **dnd5e** | — | **none at all** |

Six shapes, and none in the system a person is most likely to be holding a
sheet for. There was no uniform way to ask a system "what can this character
roll, and how?".

---

## Decision

**One new manifest declaration, `checks`, uniform across every pack**, read by
the server and never by the sheet.

Each entry names an `id`, a `label`, an optional `group`, a `formula` in
`thunderforge_dice` syntax, and `bindings` — a map from the formula's
placeholders to values published about the actor:

```json
{
  "id": "strength",
  "label": "Strength",
  "group": "abilities",
  "formula": "1d20 + MODIFIER",
  "bindings": { "MODIFIER": { "from": "value", "id": "strengthMod" } }
}
```

A new `rollCheck(worldId, actorId, checkId)` mutation resolves the bindings
against the actor and hands the finished formula to the same authoritative
path `rollDice` already uses. **`rollCheck` takes no formula argument**, and
an SDL guard asserts it never grows one: a mutation that accepted a formula
from a sheet would be ADR-044's boundary with a hole in it, wearing a
different name.

Three consequences follow from the shape rather than being decided separately:

- **A binding looks a number up; it does not compute one.** `strengthMod`
  exists because 5e's own ruleset derives it. The arithmetic
  (`(score - 10) / 2`) stays in the pack that owns the rule, and this contract
  only reads the result — which is the same line spec 029 drew.
- **A placeholder with no binding is not nought.** An unfilled sheet is the
  absence of a number, not the number zero, and the roll is refused. This is
  the call `DeclaredValues::integer` already makes.
- **A pack that declares no checks offers no check** (FR-037). Seven of the
  eight bundled packs ship that way today. The absence is a fact about the
  ruleset, not an omission for the app to fill in with a guess.

`dnd5e` gains a `checks` block generated from the `abilities` and `skills` it
already declares — each skill already names its governing ability, so nothing
new had to be authored, only expressed.

### The six existing per-pack keys are left exactly as they are

`coreCheck`, `actionRoll`, `taskResolution`, `ladderRoll`, `skillRoll` and
`manifestationRoll` stay. They describe a system's *core resolution mechanic*
— a fact about the ruleset that other things read — and `checks` describes
*what a character can be asked to roll from a sheet*. Those are different
questions that happen to have similar answers in one or two systems.

Rewriting six packs to unify them is not this feature's business, and a
migration that touched every bundled system to serve one new button would be
the largest change in the feature and the least examined.

---

## Alternatives Considered

**Read the existing per-pack keys directly.** Six shapes to understand, one of
them absent in 5e, and the understanding would have to live in the sheet —
one system's rules in shared presentation code. Rejected by the constitution
before it was rejected on effort.

**Compute the check in the sheet from `abilities` and `skills`.** This is
`(score - 10) / 2` living in the web app. It is 5e's rule, it is wrong for
every other system, and it is the exact thing spec 029 purged from the engine.

**Let the sheet send a formula and have the server roll it.** The shortest
path, and it hands the client the one decision ADR-044 exists to keep from it.
A client that names a formula names the outcome's distribution.

---

## Consequences

**Good**

- A sheet button in any system is one call with an id in it. The sheet renders
  what the system declared and offers nothing when it declared nothing.
- The trust boundary is unchanged and now has a guard that says so: `rollCheck`
  cannot take a formula without failing its SDL test.
- Adding checks to the other seven packs is per-pack, additive, and needs no
  code change — which is what a published author contract should feel like.

**Costs**

- Two ways to describe a roll now exist in a manifest, and an author has to
  know which question each answers. `packs/systems/README.md` carries that
  distinction as part of the author contract.
- `CheckDeclaration` does not validate its `formula`: the crate compiles for
  `wasm32` and deliberately does not depend on the dice crate. A formula that
  does not parse is refused at roll time, and a native test walks every
  shipped pack so a bundled pack cannot ship one.
- A malformed entry is dropped rather than raised, for the reason `resolve`
  drops an undeclared derivation: it is a build-time mistake in a bundled
  pack, not something a player at a table can act on, and a button that cannot
  be rolled is worse than no button.
