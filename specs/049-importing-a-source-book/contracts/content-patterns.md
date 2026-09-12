# Contract: Content Patterns

**Feature**: 049 | **Phase**: 1 | **Spec**: FR-010 to FR-016

What a system pack declares so that shared code can find that system's content
in a book without learning a word of that system's vocabulary.

This is a **manifest extension point**, and it follows the `vision` block spec
045 added, in the same three places (research §3): schema and validation in
`crates/pack_system_spec`, the runtime type in
`crates/thunderforge-canvas-core`, and a loader in `src/server/src` that reads
one key out of `packs/systems/<id>/system.json` and falls back to a default
when it is absent or malformed.

---

## The declaration

A top-level `contentPatterns` array in `system.json`, beside `vision` and
`movement`. Absent means this system cannot have a book read into it, and the
import is refused with that as the reason before any file is read (FR-015).

```json
"contentPatterns": [
  {
    "kind": "creature",
    "shape": "anchored",
    "anchor": "Armor Class",
    "name": { "position": "before", "withinLines": 6, "prefer": "largest" },
    "fields": [
      { "key": "armorClass",  "label": "Armor Class", "as": "integer" },
      { "key": "hitPoints",   "label": "Hit Points",  "as": "integer" },
      { "key": "speed",       "label": "Speed",       "as": "text" },
      { "key": "challenge",   "label": "Challenge",   "as": "text" }
    ]
  },
  {
    "kind": "spell",
    "shape": "anchored",
    "anchor": "Casting Time",
    "name": { "position": "before", "withinLines": 3, "prefer": "largest" },
    "fields": [
      { "key": "castingTime", "label": "Casting Time", "as": "text" },
      { "key": "range",       "label": "Range",        "as": "text" },
      { "key": "components",  "label": "Components",   "as": "text" },
      { "key": "duration",    "label": "Duration",     "as": "text" }
    ]
  },
  {
    "kind": "classFeature",
    "shape": "prose",
    "name": { "style": "bold", "endsAt": "nextName" }
  }
]
```

## Fields

| Field | Required | Meaning |
|---|---|---|
| `kind` | yes | What this pattern finds. Open — a system names its own kinds. Shared code never switches on the value. |
| `shape` | yes | `anchored` or `prose`. Decides which reader runs and what an entry may hold (FR-001a, FR-001b). |
| `anchor` | `anchored` only | The label that unambiguously begins an entry of this kind. |
| `name` | yes | How the entry's name is found. |
| `fields` | `anchored` only | The labels to read, and what each one is. |

### `name`

For `anchored`: `position` (`before` \| `after`), `withinLines` (how far to
look), `prefer` (`largest` \| `bold` \| `nearest`). The 5e creature rule —
walk backwards up to six lines from `Armor Class` and take the largest text —
is expressible exactly, which is the test this design had to pass.

For `prose`: `style` (`bold` \| `heading`) and `endsAt` (`nextName` \|
`nextHeading`). A prose entry runs until the next thing that looks like a name.

### `fields[].as`

`integer` \| `number` \| `text`. Nothing richer. A value that will not parse as
its declared type is recorded **uncertain**, with the text as read — never
dropped, never coerced.

---

## Rules a pack must satisfy

- **An anchor must be unambiguous within the book.** A label that also appears
  inside an entry's prose will start entries in the middle of other entries.
  `Armor Class` qualifies; `Range` alone would not, which is why the spell
  pattern anchors on `Casting Time`.
- **Anchors must not collide across kinds** in one system. Validation rejects
  two patterns with the same anchor.
- **`kind` must be unique** within a system.
- **A `prose` pattern must not declare `fields`.** Rejected by validation
  rather than ignored — FR-001b is a promise that prose carries no mechanics,
  and a silently ignored field list is a promise somebody thinks they have.

Validation lives beside the existing `validate_system_manifest` rules, in the
same crate and the same style.

---

## What the declaration deliberately cannot express

**Per-attack reach.** The 5e reader extracts a reach in feet out of an attack's
free prose (`Action { name, text, reach_feet }`), and spec 045's playtest
established that reach is per-attack rather than per creature size — so it
matters and cannot be dropped.

It is not a labelled field and no reasonable declaration language reaches it.
It therefore stays **in the pack, as a contribution**, using the mechanism that
already exists for behaviour a pack owns that data cannot express:
`SystemContribution` in
`crates/thunderforge-canvas-core/src/system_contribution.rs`, collected through
`inventory` and linked by `src/app/src/system_packs.rs`.

The shape: a pack may contribute an optional **refinement** over the generic
anchored result for a given kind — it receives what the shared reader produced
plus the lines it came from, and returns a refined entry. Shared code still
names no system. The 5e pack still owns the one thing only 5e knows.

This is FR-016's "name what the declaration cannot express rather than quietly
preserving it", and this section is that naming.

---

## The build check this must not break

`scripts/check-system-registry.mjs` scans `src/server/src`, `src/app/src` and
`apps/web/src` and fails if any file is **named** for a bundled system or
contains a bundled system's id **in double quotes**. `packs/systems/*` is
outside those roots by construction.

`KNOWN` in that script is currently empty — nothing in the repository is
exempted — and the check also fails on a stale exemption. **This feature adds
no entry to it.** If a phase cannot pass without one, the design is wrong and
the phase stops rather than the check being widened.
