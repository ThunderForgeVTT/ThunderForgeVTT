# ADR-096: Content Patterns as a Manifest Extension Point

**Date:** 2026-09-12
**Status:** **PROPOSED**
**Participants:** ThunderForgeVTT Team
**Related:** spec 049 (FR-010 … FR-016), spec 050, ADR-027 (the pack manifest contract), spec 045's `vision` block, spec 016 (`legal`)

---

## Problem Statement

`packs/systems/dnd5e/server/src/statblock.rs` reads creatures out of a book,
and it works: 451 creatures and 538 per-attack reach values out of eight
bestiaries, 2155 creatures across the whole 246-book corpus. It works for one
reason — **armour class is an unambiguous anchor.** Every 5e creature has one,
it is always labelled, and that label appears nowhere else in a statblock.

It also hard-codes `"armor class"`, `"hit points"` and `"challenge"`.
Pathfinder says AC, hp and CR. The next system says something else again.

Shared code cannot learn all three. `scripts/check-system-registry.mjs` fails
the build if any file under `src/server/src`, `src/app/src` or `apps/web/src`
is named for a bundled system or quotes its id, and its `KNOWN` exemption map
is currently **empty** — nothing in the repository is exempted. Putting a
system's vocabulary into the importer would need the first exemption, and an
exemption is how a rule stops being a rule.

So: **where does a system say what its content looks like, and in what shape?**

---

## Decision

**A system declares `contentPatterns` in its own `system.json`, and shared
code reads the declaration rather than the words — in the same three places,
and the same shape, as the `vision` block spec 045 established.**

| Where | What | Precedent it copies |
|---|---|---|
| `crates/pack_system_spec` | The manifest **schema**, plus the rules JSON Schema cannot express | `SystemVision`, `validate_system_manifest` |
| `crates/thunderforge-canvas-core/src/content_patterns.rs` | The **runtime type** | `vision_declaration.rs` |
| `src/server/src/content_patterns.rs` | The **loader** — read one key, treat absent/malformed alike | `vision_profiles.rs` |

A pattern says: the `kind` it finds (the system's own word, never switched on
by shared code), its `shape`, the `anchor` that begins an entry, how the
`name` is found, and which `fields` to read.

### Two shapes, because real books have two

Measured, not assumed:

- **Anchored** — a block of labelled fields after an unambiguous label.
  Creatures (`Armor Class`) and spells (`Casting Time`).
- **Prose** — a name and paragraphs, with no labels and no fixed fields.

A prose pattern **may not declare fields**, and validation refuses one that
tries. Spec 049 FR-001b promises prose carries no invented mechanics; a field
list that were silently ignored would be a promise somebody thinks they have.

### A prose kind must say what confirms it — added on evidence

The first version of this decision let a prose pattern be nothing but "a
heading, then paragraphs". Measured against the corpus, that description
matches **every section of every book**: the reader returned 72,974 magic
items across 246 books, including `Table of Contents` and `About`, and
returned the *identical* number for feats, because two prose kinds declared
that way are indistinguishable from each other.

So a prose pattern must also declare `confirmedBy` — phrases, one of which
must appear within a few lines of the name. A magic item is confirmed by its
type line (`Wondrous item`, `Weapon (`, `, uncommon`); a feat by
`Prerequisite`. Validation refuses a prose kind without one, and the reader
refuses it again at runtime, because the failure mode is not "a few extra
results" but the whole book, silently.

### What this costs: 5e cannot declare class features

`layout::Line::bold` is true only when **every** run on a line is bold, and a
class feature is a bold run-in name followed by roman prose — `Rage. In
battle, you…` — so such a line reads as ordinary text. Declaring the kind as a
heading instead matches everything (42,295 entries, measured), and there is no
marker line under a class feature to confirm it by.

Catching a run-in name needs sub-line run data, which the layout pass
deliberately collapses into a line. Until that changes the 5e pack declares no
`classFeature` kind at all, and spec 049's FR-013 is amended to say so. Zero is
the honest answer; 42,295 wrong ones is not.

### What measuring changed about this decision

Spec 049's prose says spells, magic items and creatures "share a shape — an
anchored block of labelled fields". Two of those three held up and one did
not. Read out of real books on 2026-09-12:

- a **magic item** is a heading name followed by an *unlabelled* type and
  rarity line — `Weapon (whip), uncommon` — then prose;
- a **feat** is a heading name followed by a `Prerequisite:` line, then prose.

Neither has an anchor label, so neither can be read as anchored content. Both
are declared **prose**, and spec 049's sentence is corrected rather than the
declaration being bent to fit it. This is the ordinary outcome here: every
parser defect found while building this reader produced text that looked
plausible and was wrong, and the corpus is the arbiter.

### What the declaration deliberately cannot express

Per-attack **reach**. `statblock.rs` extracts a reach in feet out of an
attack's free prose, and spec 045's playtest established that reach is per
attack rather than per creature size — so it matters and cannot be dropped.

It is not a labelled field, and no reasonable declaration language reaches it.
It therefore stays **in the 5e pack**, as a refinement contributed through
`SystemContribution` (`crates/thunderforge-canvas-core/src/system_contribution.rs`),
which is the mechanism that already exists for behaviour a pack owns that data
cannot express. Shared code still names no system; the pack still owns the one
thing only that system knows.

Naming this is the decision. Spec 049 FR-016 asks that whatever the
declaration cannot express be *named* rather than quietly preserved, because a
second reader kept alongside the generic one disagrees with it eventually, and
the disagreement shows up as a creature that imports differently depending on
which path ran.

---

## Alternatives Considered

**A pattern language rich enough for reach.** Regular expressions in the
manifest, say. Rejected: it moves system-specific knowledge into a config
string where it is harder to test and still system-specific, and a language
expressive enough for this is a language — a much larger thing to own than a
function pointer a pack already knows how to submit.

**Declare patterns in the pack's Rust crate via `inventory` instead.**
Rejected: the reading happens in the **browser** (spec 049 FR-020, and spec
048's record of why that is safe), and a declaration in server-side Rust
cannot be read there. The manifest is already shipped to the client.

**Import the runtime type into `pack_system_spec` instead of duplicating it.**
Rejected, and not by this ADR: `SystemVision` and `SystemResource` already
duplicate deliberately, and their comments give the reason — that crate is the
manifest *schema*, published to system authors, and importing an engine type
would drag the engine's dependency graph into every pack that only wants to
describe itself. Reversing that is a separate decision with its own
justification. A test keeps the field names honest.

**Keep `statblock.rs` as a parallel path for creatures only.** Rejected by
FR-016, and rightly. It is also cheap to retire: `statblocks()` has **no
production caller** today — only an example and its own tests — so nothing
regresses.

---

## Consequences

- A second game system becomes a **pack change**, not a shared-code change.
  Spec 049 FR-014 asks exactly this of Pathfinder 2e.
- `check-system-registry.mjs` keeps passing with an empty `KNOWN` map. If a
  phase of spec 049 cannot pass without an exemption, the design is wrong and
  the phase stops rather than the check being widened.
- The manifest contract grows a block, so ADR-027's contract and
  `packs/systems/README.md` gain it.
- A system that declares nothing **cannot have a book read into it**, and is
  told so before a file is opened. That is a refusal, not a fallback — unlike
  `vision`, where declaring nothing yields ordinary sight and is a correct
  answer. Reading a book with another system's vocabulary has no correct
  answer.

## Open

Found while implementing this and **not caused by it**: no shipped pack
manifest passes `validate_system_manifest` at all — `packs/systems/dnd5e/system.json`
carries `author` and `packages` where the schema requires `authors` and
`packs`, which means the validator has never been run against a real pack.
Tracked separately; this ADR's tests validate the `contentPatterns` block
specifically for that reason, and say so where they do it.
