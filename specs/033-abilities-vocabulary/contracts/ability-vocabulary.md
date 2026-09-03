# Contract: What a system says about abilities

**Spec**: `specs/033-abilities-vocabulary/spec.md` | **Date**: 2026-09-03

Author-facing. A system pack declares this in its `system.json`; nothing else is
required, and every part is optional.

## The block

```json
"abilityVocabulary": {
  "umbrella": { "label": "Spell", "pluralLabel": "Spells" },
  "types": [
    {
      "id": "spell",
      "label": "Spell",
      "pluralLabel": "Spells",
      "order": 0,
      "binds": "character",
      "grade": { "label": "Level", "min": 0, "max": 9 }
    },
    {
      "id": "enchantment",
      "label": "Enchantment",
      "pluralLabel": "Enchantments",
      "order": 1,
      "binds": "item"
    }
  ]
}
```

| Key | Meaning | Absent means |
|---|---|---|
| `umbrella.label` / `pluralLabel` | The system's word for the concept itself, replacing "Ability"/"Abilities" everywhere a person reads it | The application's word |
| `types[].id` | Stable identity. Matches a built-in to re-label it; anything else adds a type | — (required) |
| `types[].label` / `pluralLabel` | What a person reads | The id, never blank |
| `types[].order` | Display order of the tab | Declaration order, after the built-ins |
| `types[].binds` | `character`, `item`, or `nothing` — **exactly one**, never a list | `character` |
| `types[].grade` | `{ label, min, max }` — the system's word for an ordered value and its range | The type is ungraded, and no surface shows a grade for it |

## The four built-in types

`spell`, `feat`, `power`, `talent`. **Permanently available** for authoring
(FR-017) — declaring one of these ids re-labels it, and never creates a second
tab.

**A built-in is shown when the system uses it, or when the world holds one**
(FR-011a). A built-in the active system neither declares nor re-labels, and of
which the world holds no abilities, gets no tab. So a 5e pack declaring Spells,
Feats and Enchantments does not saddle every 5e world with permanently empty
"Powers" and "Talents" tabs — and no ability is ever hidden by this rule,
because holding one is itself enough to show the tab.

A system that declares nothing gets all four, correctly labelled, in a complete
tab set. That is not a degraded mode — it is what most systems will want.

## Rules a declaration is held to

1. **Re-labelling is by id.** `{"id": "spell", "label": "Scroll"}` makes the
   Spell tab read "Scrolls". One tab.
2. **A malformed entry is skipped, and the rest survives.** A `types` entry
   that is not an object, or has no `id`, is ignored; the other entries still
   apply. A pack does not lose its whole vocabulary to one typo.
3. **No declaration can produce a blank label.** Missing or empty labels fall
   back to the id.
4. **A type declared by this system is not offered in a world running a
   different one** (FR-013). Content authored under it is not lost — see
   *Unrecognised types* below.
5. **An irreconcilable identity collision is reported when the vocabulary is
   assembled** (FR-015), not when a GM first tries to author one.
6. **Grade ranges are checked at authoring, not at storage.** A value already
   recorded outside a *newly* declared range is retained and displayed, never
   clamped or discarded (FR-023). Narrowing a range does not edit anybody's
   content.

## Backwards compatibility with `abilityFacets`

The existing block still works:

```json
"abilityFacets": { "spell": { "label": "Scroll", "pluralLabel": "Scrolls" } }
```

It is read as a `types` list carrying labels only — no ordering, no facets, no
new types. `packs/systems/genie/system.json` ships it today and continues to
work untouched. Where both blocks are present, `abilityVocabulary` wins and the
older block is ignored for the ids it covers.

## Unrecognised types

An ability keeps the type it was authored under, forever, whatever system the
world is running. When the active system does not recognise that type, the
ability:

- stays listed, viewable, editable and deletable by its GM;
- appears in a **final tab** in the same tab set, present only while such
  abilities exist, clearly marked, and offering no creation — FR-013 forbids
  authoring a type the active system does not recognise;
- is labelled with the **stored identity itself**, shown plainly. No other
  system's manifest is consulted to prettify it, and no label is copied onto the
  ability at authoring time: the first reads a system this world is not running,
  and the second duplicates the manifest into content, where it goes stale as
  soon as that pack re-labels the type;
- returns to its own tab, with the system's labels, when a system recognising
  it is active again;
- is **never** re-typed automatically. A GM may re-type it deliberately, and
  nothing else may.

This is a presentation state. Nothing about the stored ability changes when it
enters or leaves it.

## What this block may not do

It declares **names and shapes**, never behaviour. A type may say *that* it
binds to items and *that* it is graded; the application performs both
generically for every system. There is nowhere in this format to put a rule, a
formula or a conditional, and that is the boundary rather than an omission — it
is what keeps a pack from outside the product to data (ADR-029), and what keeps
a world's content portable when its system changes.
