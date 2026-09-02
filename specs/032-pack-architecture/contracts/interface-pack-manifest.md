# Contract: `interface.json`

What an interface pack author writes, and what validation guarantees a reader.
This is the whole surface — there is no second file, no stylesheet, and no
module. A pack that wants to do something not expressible here is asking for
behaviour, and the answer is no (FR-003).

Location: `packs/interface/<id>/interface.json`. The directory name and the
manifest's `id` MUST match; validation rejects a mismatch, because a pack whose
identity depends on which of the two you read is a pack that can be referred to
two ways.

## Shape

```jsonc
{
  "id": "forge",                       // required; [a-z0-9-]+, matches directory
  "type": "interface",                 // required; the ONLY accepted value (FR-002)
  "title": "Forge",                    // required; what a Game Master sees
  "version": "1.0.0",                  // required; semver
  "description": "…",                  // required; one sentence
  "compatibility": {                   // required; same shape as system.json
    "minimum": "0.1.0",
    "verified": "0.1.0",
    "maximum": null
  },
  "legal": { … },                      // required; same shape as system.json's
                                       //   SystemManifestLegal, reused verbatim
  "light": { /* tokens */ },           // required
  "dark":  { /* tokens */ },           // required — see research.md §5
  "canvas": { /* AppearanceOverride */ }, // optional; absent = engine defaults
  "targets": ["genie"],                // required; [] means "any system"
  "layout": { /* see Layout below */ } // optional; absent = generic default
}
```

`deny_unknown_fields` applies at every level. A key this contract does not name
is a rejection, not an ignored value — an author who misspells `background`
finds out at validation rather than by looking at a screen that is subtly wrong.

## The token set

`light` and `dark` each take the same keys. Every key is optional; an absent key
keeps the base pack's value for that mode, so a pack that only wants to change
the accent colour is four lines long.

| Group | Keys |
|---|---|
| Ground | `background`, `foreground` |
| Surfaces | `card`, `cardForeground`, `popover`, `popoverForeground` |
| Emphasis | `primary`, `primaryForeground`, `secondary`, `secondaryForeground`, `accent`, `accentForeground` |
| Recessive | `muted`, `mutedForeground` |
| Signal | `destructive` |
| Edges | `border`, `input`, `ring` |
| Charts | `chart1` … `chart5` |
| Sidebar | `sidebar`, `sidebarForeground`, `sidebarPrimary`, `sidebarPrimaryForeground`, `sidebarAccent`, `sidebarAccentForeground`, `sidebarBorder`, `sidebarRing` |
| Geometry | `radius` |

These are the custom properties `apps/web/src/styles/globals.css` already
defines; the camelCase key maps to the `--kebab-case` property one-for-one. The
list is the vocabulary because it is *already* the vocabulary — a pack cannot
introduce a token the application does not consume, which is another way FR-003
holds without anyone policing it.

**Colour values** are CSS colour strings. `oklch(...)`, `#rrggbb`,
`rgb(...)`, and `oklch(... / <alpha>)` are accepted; anything the validator
cannot parse into a colour is rejected, because a value it cannot parse is a
value it cannot check the contrast of. **`radius`** is a CSS length.

**Deliberately absent**: anything positional (`display`, `position`, `z-index`,
`opacity` on a container, `pointer-events`), any selector, any URL, and any
font file. The first group is FR-012 — a look must not be able to move a control
out of reach. The last is scope: web fonts are a real want and a separate
decision, involving a fetch this format currently does not make.

## The canvas block

`canvas` is an `AppearanceOverride`, unchanged from
`crates/thunderforge-canvas-core/src/resource_display.rs`:

| Key | Meaning |
|---|---|
| `track`, `trackAlpha` | The unfilled part of a status bar |
| `undisclosed` | Fill for a value the viewer is not being told |
| `palette` | Fill colours, taken in the order a system declares its resources |
| `barHeight`, `barGap`, `firstBarOffset` | Bar geometry, in world units |

Colours here are `[r, g, b]` floats in 0.0–1.0, matching the engine's `Rgb`.
This differs from the CSS half's colour strings and that is not an oversight:
the two halves are consumed by two renderers and converting between them in the
manifest would put a conversion in the one place nobody would test it.

## Targets

`targets` is the systems this pack is built for, by manifest `id`. It is a
claim, and it is checked (FR-026): validation reads each named system's
manifest and rejects the pack if its layout references an identifier that
system does not declare, naming both the identifier and the system. A claim
nobody verifies is the failure spec 016 already corrected once, for legal
metadata.

An **empty** `targets` means the pack composes against any system. A pack may
only declare it if its layout uses generic addressing exclusively — naming an
identifier is naming a system, whatever the list says. Forge declares `[]`
(FR-025b).

Validated **per named system, independently** — never against their union. A
pack targeting `dnd5e` and `blades_in_the_dark` must work for each, and
`hitPoints` existing in one does not excuse referencing it while rendering the
other.

## Layout

A layout is a tree of constructs. Every construct addresses the system's
declarations in one of two ways, and the difference is the whole design
(FR-025a):

**Generic** — addresses a declaration *set* by kind and declaration order, and
names nothing:

```jsonc
{ "kind": "badgeGrid", "of": "attributes", "columns": 3 }
{ "kind": "barStack",  "of": "resources" }
{ "kind": "rowList",   "of": "skills", "showProficiency": true }
```

**Specific** — names identifiers the target system declares:

```jsonc
{ "kind": "tracker", "id": "deathSaves", "boxes": 3, "rows": 2 }
{ "kind": "slotGrid", "of": "spellSlots", "levels": 9 }
{ "kind": "pair", "value": "strength", "beside": "strengthMod" }
```

Constructs nest inside `section`, `column` and `row` containers, each with a
`title` and an optional `collapsed` default. A construct addressing a set the
system declares as empty renders nothing rather than an empty frame — Fate Core
declares zero abilities, and a pack should not draw a heading over nothing.

**What a layout cannot do**, and this is the FR-003a line applied concretely:

- No expressions. `"value": "strength"` is a reference; `"value": "(strength -
  10) / 2"` is a computation, and belongs to the system's own rules.
- No conditionals. A construct cannot render differently depending on a value.
- No thresholds, no colour ramps keyed to a number, no "red below 25%". That is
  a rule about what a number *means*.
- No text the system did not declare. Labels come from declarations, so a pack
  cannot rename a system's concepts or slip in wording of its own.

The last one is worth stating twice. A pack that could supply its own labels
could reproduce a publisher's terminology and section headings, which FR-003b
forbids and which is the more likely route to that failure than layout geometry.

## Validation

Run before a pack is made available. Every failure names the offending value.

1. **Structural** — required keys present, `deny_unknown_fields`, `id` matches
   the directory, `type` is `"interface"` (FR-002).
2. **No behaviour** — implied by 1. There is no key that can carry code, so
   this is a property of the schema rather than a check. Stated here because
   FR-003 requires the guarantee to be nameable, and "the format cannot express
   it" is a stronger guarantee than a scan.
3. **Colour parse** — every colour value resolves. An unparseable colour is a
   rejection, not a fallback.
4. **Legibility floor (FR-012a)** — for each mode independently, every declared
   foreground/background pair meets WCAG AA: 4.5:1 for text pairs
   (`foreground`/`background`, `cardForeground`/`card`,
   `popoverForeground`/`popover`, `mutedForeground`/`background`,
   `sidebarForeground`/`sidebar`, and each `*Foreground` against its emphasis
   colour), 3:1 for non-text (`border`, `input`, `ring` against their
   backgrounds). A failure names the pair, the computed ratio, the required
   ratio, **and the mode** — a pack that reads fine in dark and fails in light
   is common enough that "this pack failed" alone would send an author looking
   in the wrong place.
5. **Legal** — the same `validate_legal_content` the system-pack path already
   runs. A pack is a redistributable artifact whoever wrote it.
6. **Targeting (FR-026)** — for each id in `targets`, read that system's
   manifest and confirm every identifier the layout names is declared by it,
   either as a stored declaration or in that system's `derived_declarations`.
   A failure names the identifier **and** the system. A pack declaring
   `targets: []` fails if its layout names any identifier at all.
7. **Conformance, for Forge only (FR-007a)** — every construct the format
   offers appears somewhere in Forge, and Forge names no identifier. A format
   construct nothing can build is then caught by Forge's own test rather than
   by an author discovering it a year later.

## Worked minimum

The smallest pack that validates:

```json
{
  "id": "high-contrast",
  "type": "interface",
  "title": "High Contrast",
  "version": "1.0.0",
  "description": "Maximum separation between text and ground.",
  "compatibility": { "minimum": "0.1.0", "verified": "0.1.0", "maximum": null },
  "legal": {
    "licenseName": "AGPL-3.0-or-later",
    "attributionText": "ThunderForgeVTT Contributors"
  },
  "light": { "background": "#ffffff", "foreground": "#000000" },
  "dark":  { "background": "#000000", "foreground": "#ffffff" },
  "targets": []
}
```

No `layout`, so it inherits Forge's — which is generic, so this pack works with
every system while changing only its colours. That is the floor the format is
designed around: a pack that wants to change one thing changes one thing.
