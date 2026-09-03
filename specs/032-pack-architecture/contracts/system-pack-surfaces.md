# Contract: what a system pack contributes (Increment F)

Design contract for User Story 2. **This is not the document FR-015 asks for** —
that one is author-facing and ships at `packs/systems/README.md`. This one is
for us, and records the shape the author-facing document will describe.

FR-015 and SC-010 are specific: an author must be able to produce a working
pack **from the published contract alone, without reading shared application
source**, and the contract must have **zero references to documents that do not
exist**. Both are testable, and the second is the same shape as
`check-system-registry.mjs`.

---

## 1. Declarations — data, and already shipped

A pack declares these in `system.json`. All of it exists today, read
server-side, with no pack code involved. The author-facing document must
describe every block, because an author cannot read `sheet.rs` to find them.

| Block | Read by | What it declares |
|---|---|---|
| `abilities` | `attributes.rs` | Ability scores, with label, abbreviation and order |
| `resources` | `status_display.rs` | Pools, with current/max sources and an optional `group` |
| `movement` | `declared_values.rs` | Speeds, with a source field, a default and an order |
| `sheet` | `sheet.rs` | Text, list, number, track, state and player-named slots |
| `groups` | `sheet.rs` | A group's own label and its headline member |
| `turnStructure` | `turn_structure.rs` | Whether the ruleset counts rounds, and what it calls them |
| `legal` | `systems.rs` | Licence and attribution, already served |

**Rule**: a declaration is a *shape of value*, never a rule about one. The
moment a block carries a threshold or a condition, the format is a language and
FR-003 is gone. This is the line Increment E held and the document must state
it.

## 2. Computation — code, bundled only

`SystemRules` in `thunderforge_canvas_core::system_rules`, implemented by the
pack's own server crate, submitted through `inventory`.

- `derived_declarations()` — the identifiers `derive` may return, declared up
  front so an interface pack can be validated against a system without running
  it.
- `derive(&DeclaredValues) -> Vec<DeclaredValue>` — **pure**. No database, no
  network, no clock, no randomness. A derived value is recomputed on every read
  and never written down.

Already shipped: `genie` derives Wish Points by level, `dnd5e` derives
modifiers, saves, skills and passive Perception.

## 3. Behaviour hooks — code, bundled only, **shape open**

The new surface in this increment, and the one research F-2 has not settled.

### `on_world_created`

Called inside the transaction that creates a world on this pack's system.

- **Why in-transaction**: a hook that commits separately can leave a world
  without the row its system expects, and the failure appears later as a
  missing session rather than a failed creation.
- **Failure**: aborts world creation. A half-created world is worse than a
  refused one.
- **Only implementer today**: `genie`, inserting `world_genie_sessions` with
  `doom_clock_max: 6` — currently a hardcoded branch in `graphql.rs` and the
  last entry in `check-system-registry.mjs`'s `KNOWN` list.

**Open**: whether the hook receives a database connection (requiring `diesel`
in pack crates, and a decision about who owns the pack's tables and
migrations), or returns data the server persists. See research F-2.

## 4. Surfaces — mounted, contained, named

A pack-contributed surface is mounted wherever that system's content is
encountered. Today the only such surface is the character sheet, and it is
rendered from *declarations* rather than pack code — which is why scenarios 1
to 3 already pass.

What is missing is containment (FR-016, SC-009):

- A failure inside a surface **must not** take the session. Measured as: 100%
  of the surrounding session remains usable.
- The message **must name the responsible pack**, in 100% of cases. Not "an
  error occurred" — *which pack*, and *what is unavailable*.
- `apps/web` has no error boundary at all today, so this is new machinery.

## 5. Installation

Per **ADR-029**: outside code is not executed, and executable extension is
bundled-only. A pack from any other source is data or is refused.

Acceptance scenario 4 — "accepted only if the security and sandboxing terms are
satisfied, and rejected with a stated reason otherwise" — therefore reduces to:
a pack that is not shipped with the product is refused, and the stated reason is
that this product does not run code it did not compile. The interim framing in
FR-017 is now the decision.

---

## What the author-facing document must contain

`packs/systems/README.md`, modelled on `packs/interface/README.md`:

1. Every declaration block in §1, with a worked example per block.
2. The `SystemRules` contract in §2, including the purity rule and why.
3. Every hook in §3, with its transaction and failure semantics.
4. What a pack may **not** do, and that this is enforced by the format rather
   than by review.
5. Where a pack lives, how it is discovered (`inventory`, plus one
   `use <pack> as _;` line — the linker fact from `system_contribution.rs`),
   and why that line is not a registry.

**Testable**: every path and document this file references must exist. A pack
author who follows a dangling reference has been failed by the contract, which
is what SC-010's second clause measures.
