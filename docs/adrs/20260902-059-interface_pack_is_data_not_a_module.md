# An Interface Pack Is Data, Not a Module

- **Date**: 2026-09-02
- **Status**: Accepted
- **Spec**: `specs/032-pack-architecture/` (FR-003, FR-003a, FR-003b, SC-011)

## Context

Spec 032 describes two kinds of pack. A **system pack** contributes a ruleset —
resources, actor fields, a character sheet. An **interface pack** changes how
ThunderForge looks: colours, the engine's bar appearance, where the fields of a
sheet sit.

Only one of them can ship, and the reason is a document that does not exist.
`docs/adrs/20260504-029-runtime_module_loading_and_security.md` is an **empty
file** — zero bytes. It is the ADR that is supposed to record what pack-supplied
code may reach, what it is denied, and how it is contained, and it records
nothing at all. Nothing in this repo currently answers the question.

A system pack cannot route around that, because contributing a character sheet
*is* contributing behaviour — something has to decide what a derived field
computes and when a control is enabled. An interface pack can route around it
completely, and there are exactly two ways to arrive there:

1. Let a pack ship code, then constrain what the code can do. This requires
   ADR-029 to be written, argued, implemented and tested first.
2. Give the format nowhere to put code. This requires nothing, because there is
   no execution to contain.

Only the second lets the interface half of spec 032 ship ahead of the system
half, which is what SC-011 commits to.

The stake rose during clarification. When an interface pack was only a palette,
"no code" was trivially true — there was nothing to compute. The pack now also
declares **layout** over a character sheet, and the pressure to add "just one
conditional" is real. It will not arrive as a bad idea; it will arrive attached
to a genuine rendering problem that a conditional would solve in four lines.

## Decision

**An interface pack is a single JSON manifest of values.** It carries colour
tokens, an engine appearance override, and a declarative layout. It contributes
no JavaScript, no stylesheet, no ES module, and nothing that executes. There is
no field in the format whose value is code, so there is nothing to sandbox.

### The mechanism already exists

`AppearanceOverride`, in
`crates/thunderforge-canvas-core/src/resource_display.rs`, is the shape an
interface pack's engine section takes: a partial, serde-deserialised struct of
optional fields, folded onto a base by `apply_to`, where an absent field means
"leave this alone" rather than "reset this".

Its derive is the load-bearing part:

```rust
#[serde(rename_all = "camelCase", deny_unknown_fields)]
```

`deny_unknown_fields` makes a key the contract does not name a **rejection**,
not an ignored value. An author who misspells a token finds out at validation,
not by looking at a screen that is subtly wrong in a way nobody can attribute.
It also means the format cannot be extended from the outside: a pack cannot
smuggle in a field by writing one down.

### The line, in a form that survives pressure (FR-003a)

> Declaring **where** a value appears is presentation. Declaring **what** a
> value is, is behaviour.

"Show `strengthMod` next to `strength`" is layout, and an interface pack may say
it. "Show `(strength - 10) / 2`" is a computation, and it belongs to the system
that defines what a modifier is. The first is a position; the second is a rule
about a ruleset wearing a rendering problem's clothes.

### No pack-supplied stylesheet

A CSS file is not inert. It can position a control off-screen, collapse it to
zero size, or set `pointer-events: none` — a control that is present in the DOM,
passes every structural check, and cannot be used. That is FR-012's "hide,
disable, or make unreachable" arriving through a side door, and it arrives
without a single line of script.

A fixed set of named custom properties cannot express any of those. That is not
a restriction chosen for tidiness; it is the reason the format is a list of
names rather than a language.

## Consequences

**SC-011 becomes achievable rather than aspirational.** Interface packs ship
without ADR-029, because they raise none of the questions ADR-029 is for.
ADR-029 remains empty and remains blocking — for system packs, which is where
the debt actually belongs.

**A pack cannot rename a system's concepts (FR-003b).** Labels come from the
system's declarations — `ResourceDefinition.label` and its kind — not from the
pack, so an interface pack cannot reproduce a publisher's terminology or their
section headings. This is the right copyright posture and it falls out of the
format rather than being policed. A published character sheet is read to learn
*what a system asks a player to track*, which is a fact about the ruleset;
it is never read to learn what ours should look like. The layout, the ornament,
the wording and the trade dress are the copyrighted part, and a pack has no
field in which to copy them.

**The format's expressiveness is now load-bearing, in both directions.** Too
thin, and a pack targeting a real system cannot express a nine-level spell-slot
grid, at which point authors ask for an escape hatch and the escape hatch is
code. Too rich, and it acquires conditionals — at which point it is a
programming language, ADR-029's question is live again, and this ADR is void.

**The guard is narrower than the risk.** FR-007a requires the base pack, Forge,
to exercise every construct the format offers, so an unused construct is a
failing conformance test rather than a quietly rotting one. That guards the
constructs Forge uses. It does not prove the set is sufficient for a system
nobody has written a pack for yet, and the first third-party pack is where that
gets tested for real.

**"Just one conditional" is now a decision, not a patch.** Adding one means
amending this ADR and answering ADR-029, which is the correct cost for a change
that turns a manifest into a runtime.

## Alternatives Considered

- **Let a pack ship a scoped stylesheet.** Rejected: CSS reaches FR-012's
  outcomes — off-screen, zero-size, `pointer-events: none` — with no script
  involved, so "no code" would be a claim about syntax rather than about
  capability.
- **Let a pack ship an ES module behind a sandbox.** Rejected as premature: the
  sandbox's rules are exactly the empty ADR-029, and adopting the mechanism
  before writing the rules is how the rules end up being whatever the
  implementation happened to allow.
- **A small expression language for derived fields.** Rejected: it answers the
  spell-slot-grid problem and the strength-modifier problem with the same
  feature, and the second one is the system's job. A pack that can compute is a
  pack that can disagree with the ruleset it is decorating.
- **Ignore unknown manifest keys instead of rejecting them.** Rejected: it turns
  a typo into a screen that is silently wrong, and it leaves the format's edge
  undefined — the exact opposite of what `deny_unknown_fields` buys.

## Related Decisions

- **ADR-029** — runtime module loading and security. Empty, and the direct
  reason this decision exists.
- **ADR-054** — the contribution seam; the shape a system pack's contributions
  would have to take once ADR-029 is answered.
