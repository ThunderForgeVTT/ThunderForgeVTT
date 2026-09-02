# Interface packs

A pack in this directory changes only how ThunderForge **looks**. It contributes
no behaviour: no action, no rule, no computed value, no data change, and no
executable logic (FR-003).

That is a safety boundary before it is an aesthetic one. Because an interface
pack contributes nothing that runs, it never raises the question of executing a
third party's code inside a player's session — the question ADR-029 was opened
to answer and has not. A system pack cannot avoid that question. An interface
pack avoids it by having nowhere to put code, which is why this half of spec 032
can ship while the other half waits.

## What a pack is

One file: `<pack-id>/interface.json`. Colour tokens for light and dark, an
optional canvas appearance override, an optional declarative layout, and the
systems it targets. There is no stylesheet, no module, and no second file.

The full contract is
[`specs/032-pack-architecture/contracts/interface-pack-manifest.md`](../../specs/032-pack-architecture/contracts/interface-pack-manifest.md).

## The line, for when it is under pressure

Declaring **where a value appears** is presentation. Declaring **what a value
is** is behaviour.

`"value": "strengthMod"` is layout — it points at something the system already
publishes. `"value": "(strength - 10) / 2"` is a computation, and belongs to the
system that owns the rule. The format has no place to put the second one, and
that is the point rather than an omission to be fixed later.

## The type is exclusive

A pack is an interface pack or a system pack, never both (FR-002). System packs
live in `../systems/`. This directory cannot hold one, which is the cheapest
enforcement of the rule available — the safety property attaches to the type,
so the type has to be unambiguous.

## Naming

Packs bundled with the product are named **Forged &lt;Metal&gt;** — Forged Iron,
Forged Steel — with **Forge** as the base pack (FR-007b).

Third-party packs are *not* required to adopt that convention. The name signals
that a pack ships with ThunderForge, and requiring it of packs ThunderForge did
not author would make a claim on somebody else's work.

## Forge is a peer, and is also the reference

Forge is always present and is what applies when nothing else is chosen. It has
**no capability, placement, or exemption another pack cannot have** (FR-007) —
it is discovered by the same directory listing, validated by the same rules, and
served by the same route as anything else here.

It carries one obligation the others do not, which is the opposite of a
privilege: every construct the format offers must appear somewhere in Forge, and
Forge must name no system's identifiers (FR-007a, FR-025b). That makes it the
universal fallback *and* the conformance test — a format construct nothing can
actually build fails in Forge rather than being discovered by an author a year
later. The schema, not Forge, remains the authority on what a pack may contain.

## What packs must never do

- Reproduce a publisher's sheet layout, ornament, wording, or trade dress
  (FR-003b). Published character sheets tell us **what a system asks a player to
  track**, which is a fact about the ruleset. They do not tell us what ours
  looks like. Every bundled pack is an original ThunderForge design.
- Hide, disable, or make unreachable any control the base pack presents
  (FR-012). This is why the format offers named tokens rather than free CSS: a
  stylesheet can move a control off-screen or set `pointer-events: none`, and a
  colour token cannot.
- Fall below the legibility floor (FR-012a). A pack that fails contrast is
  rejected at validation rather than shipped with a warning, because the look is
  chosen per world by the Game Master and a reader who cannot see it has no
  setting of their own to escape to.
