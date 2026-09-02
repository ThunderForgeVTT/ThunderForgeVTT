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
  "canvas": { /* AppearanceOverride */ } // optional; absent = engine defaults
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
  "dark":  { "background": "#000000", "foreground": "#ffffff" }
}
```
