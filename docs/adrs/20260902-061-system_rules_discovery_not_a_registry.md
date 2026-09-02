# A System Pack's Rules Are Discovered, Not Listed

- **Date**: 2026-09-02
- **Status**: Accepted (2026-09-02, after the spike below)
- **Spec**: `specs/032-pack-architecture/` (FR-029, SC-004)

## Context

Adding a game system to ThunderForge today means editing shared server code.
Not as an oversight — as the design.

`src/server/src/systems.rs` carries one registration function per system:

`pub fn register_dnd5e_system(registry: &mut GameSystemRegistry)` names `"dnd5e"`
as a string literal and wires five validator function pointers by hand —
`ability_data`, `resource_data`, `proficiency_data`, `trait_data`,
`spell_data`. `register_genie_system` does the same for `"genie"`. There are
seven such functions, and the file says out loud that most of them are copies:

> All five follow `register_genie_system`'s exact pattern: no spell_data slot
> (none of the five research digests found a spellcasting-specific data
> shape distinct from generic resource_data), full ability/resource/
> proficiency/trait_data validation from each pack's own validators.rs.

`GAME_SYSTEMS` then calls all seven in order, with
`// In future phases: register_coc7e_system(&mut registry);` sitting at the
bottom as the eighth line somebody will have to write.

That is one place. `src/server/Cargo.toml` is the second: seven per-pack
dependency blocks, `[dependencies.dnd5e_server]` through
`[dependencies.yze_server]`, each a `path` into `packs/systems/<id>/server`.

SC-004 measures exactly this:

> A new game system can contribute a character sheet, an item presentation, and
> rules behaviour with **zero** lines changed in shared application code —
> measured as: the change set that adds the system touches only that system's
> own pack directory.

Two edits to shared code, both required, is the failure that success criterion
names.

The same failure has a second instance on the web side.
`apps/web/src/api/gameSystems.ts` holds `BUNDLED_SYSTEM_IDS` (the seven ids
again), `BUNDLED_SYSTEM_LABELS` — titles the comment says are "mirrored from
each pack's `system.json` `title` field" — and `IMPLEMENTED_SYSTEM_IDS`, which
is `new Set(["genie"])`. Three hand-maintained lists of the same seven things,
one of them a copy of data that already exists in each pack. The file is honest
about being a stopgap; it is still a third place to edit.

## Decision

**Adding a system pack must not require editing a central registry in shared
server code.** A pack's implementation of the system contract is *discovered*.

And because that property is easy to state and easy to erode,
`scripts/check-system-registry.mjs` fails the build if a hand-maintained list of
system identifiers reappears in shared server code — modelled on
`scripts/check-interaction-seam.mjs` and added to `scripts/verify.mjs`.

**The discovery mechanism itself is not settled**, which is why this ADR is
Proposed rather than Accepted. It is the least settled decision in spec 032, and
recording it as settled would be a lie about how much is known.

## Why the check matters regardless of which mechanism wins

A behavioural test cannot catch this eroding, because **a central list that is
up to date works perfectly**. Every test passes. The failure is not a bug; it is
a maintenance burden that shows up only as the eighth system taking as long as
the first, by which point the shape is entrenched.

A check catches it in the diff instead. That is the argument ADR-054 already
made for the interaction seam, and `check-interaction-seam.mjs` states it in its
own header: "A behavioural test cannot catch that erosion until it has already
happened and something breaks. This can catch it in the diff."

Two of that script's design choices carry over unchanged. It checks for **words
rather than imports**, because "an import check would pass against a plugin that
had grown a `match` on effect id strings, which is the likeliest shape the
violation actually takes: no new dependency, just knowledge." A registry check
has the same weakness: the violation will not arrive as a new
`[dependencies.foo_server]` block, it will arrive as a `match system_id` in a
file that already compiles. And it treats false positives as a feature, because
"the alternative is a check with holes carved into it for convenience, which is
a check nobody trusts."

## The precedent is weaker than the spec assumes

Spec 032's Assumptions name "the subsystem-contributes-its-own-declarations
pattern already established for interaction effects" as the precedent, including
"the expectation that the 'no central list' property is enforced automatically
rather than by convention."

That is more than ADR-054 delivered, and this ADR should say so plainly.

`src/server/src/interaction.rs` still has a central list. `pub fn registry()`
calls `EffectRegistry::assemble(contributions())`, and `contributions()` is a
`Vec` of explicit calls — `lore_link::effects()`, `wall::interaction_effects()`,
and so on. Its own doc comment does not pretend otherwise: "Adding a contributor
is one line here plus a declaration set in the module that owns the subsystem."

What `check-interaction-seam.mjs` enforces is narrower than "no central list":
that `src/engine/src/plugins/interaction.rs` contains none of the words
`light`, `door`, `sound` — that the **core owns no effect**. That is a real and
valuable property, and it is not the one FR-029 asks for. FR-029 says
*discovered rather than listed*; the interaction seam is *listed, in one place,
with the core kept ignorant of what is on the list*.

Building on this precedent means either going further than it went, or admitting
the target is the same modest property under a more ambitious name.

## Alternatives Considered

- **A distributed-slice crate (`inventory` or `linkme`).** True discovery: each
  pack crate registers itself at link time and no list exists anywhere. The
  costs are a new dependency and linker-dependent behaviour. `wasm32` is the
  case that must be checked before committing, because this project's engine
  ships as wasm (ADR-055) and the pack crates build for both targets —
  `packs/systems/genie/engine/Cargo.toml` carries a
  `[target.'cfg(target_arch = "wasm32")'.dependencies]` block. A discovery
  mechanism that silently finds nothing under one target is worse than a list.
- **A generated list, plus the checker.** Honest and boring. A build step writes
  what a human would have written, and the checker is the thing actually holding
  the property. Weaker than FR-029's wording — a generated list is still a list
  — but it is a list nobody edits, and the failure mode SC-004 measures is
  editing, not existence.
- **Keep the hand-written list.** Rejected. It is precisely the thing SC-004
  measures, and `systems.rs` already documents its own repetitiveness.

## The spike, and what it changed

Written as Proposed because the mechanism was unsettled. It was settled the
same day by measurement, and the answer was not the one this ADR expected.

`inventory` collects fine in a single crate, and builds for
wasm32-unknown-unknown. But the case that matters failed:

| Setup | Collected |
|---|---|
| A binary depending on a submitting crate, naming no symbol from it | **nothing** — debug and release alike |
| The same, plus one `use pack as _;` | everything |

An unreferenced Rust rlib is never linked, and its submissions go with it.
Distributed slices distribute the *content* of a registration; they do not
make a crate present. Nothing in the crate's documentation is wrong about
this — it is ordinary static linking — but it means "discovered rather than
listed" cannot be literally true for statically linked packs, and a design
that assumed otherwise would have shipped a product that silently registered
no game systems at all.

**So the decision stands, with the boundary drawn where the measurement put
it.** `inventory` carries what a pack contributes — its id, its validators,
its rules constructor. `src/server/src/system_packs.rs` holds one `use <pack>
as _;` line per bundled pack, and `Cargo.toml` holds one dependency.

The distinction that makes this acceptable rather than a defeat: those two
lines are **build-graph facts**. They say a crate exists and should be linked.
They say nothing about what it contains — not its data shapes, not its
validators, not its rules — so unlike the seven `register_*_system` functions
they replaced, there is nothing in them that can drift out of step with a
pack. The thing that rots is knowledge, and the knowledge is gone.

SC-004 asks for "zero lines changed in shared application code". This delivers
two, and neither can be wrong about a system. That gap is worth stating
plainly rather than rounding down.

## What Would Change the Answer

This is Proposed because two findable facts could decide it the other way:

1. **`inventory`/`linkme` proves unreliable on `wasm32`.** Discovery that works
   natively and not in the browser is not discovery.
2. **The pack crates need to become optional Cargo features**, so a build can
   exclude a system. Link-time registration and conditional compilation of the
   registrants interact badly enough to be worth avoiding.

If either holds, the generated-list-plus-checker option becomes the right one,
and **this ADR should be superseded rather than quietly reinterpreted**. The
checker survives either way; it is the part of this decision that is not in
doubt.

## Related Decisions

- **ADR-054** — the interaction effect contribution seam; the precedent this
  decision builds on and, above, declines to overstate.
- **ADR-059** — an interface pack is data, not a module; the other half of spec
  032's pack story, and the half that ships first.
