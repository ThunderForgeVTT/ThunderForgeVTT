# Contract: Ability presentation facets

Per-system display labels for the fixed ability classifications
(FR-009..FR-014). The one genuinely new mechanism in this feature.

**No server code, no GraphQL, no database, no migration.** See research.md §1.

## Manifest contract

A new **optional** top-level `abilityFacets` key in `packs/systems/<id>/system.json`:

```json
"abilityFacets": {
  "spell":  { "label": "Spell",  "pluralLabel": "Spells" },
  "feat":   { "label": "Feat",   "pluralLabel": "Feats" },
  "power":  { "label": "Power",  "pluralLabel": "Powers" },
  "talent": { "label": "Talent", "pluralLabel": "Talents" }
}
```

Type: `Record<AbilityClassificationKey, { label: string; pluralLabel?: string }>`

- Keys are the fixed classification DB values: `spell`, `feat`, `power`, `talent`.
- `label` — singular, used for a single ability's classification badge.
- `pluralLabel` — optional; used for group headings and filter labels. Falls
  back to `label` when absent (**not** to `label + "s"` — pluralization is the
  pack author's job, since not every language or term pluralizes that way).
- **Every level is optional.** An absent block, an absent classification key, a
  non-object value, or a missing/empty `label` all fall back to the built-in
  default for that classification (FR-011).
- Unknown keys are ignored, not an error.

⚠️ The key MUST be `abilityFacets`, **not** `abilities` — all 8 shipped packs
already use the top-level `abilities` key for ability *scores*
(`{ label, abbreviation }` per score). That key is taken and stays taken
(FR-014).

### Why this validates without a change

`pack_system_spec`'s schemars-derived schema does not set
`additionalProperties: false`, so unknown top-level keys already pass
`validate_system_manifest`. `get_system_manifest` returns the parsed
`system.json` verbatim after validating only `legal`, so a new key reaches the
client with no server change. ADR-027 explicitly sanctions "any system-specific
blocks" — no ADR amendment required (research.md §1).

## Built-in defaults

| Classification | Default `label` | Default `pluralLabel` |
|---|---|---|
| `spell` | Spell | Spells |
| `feat` | Feat | Feats |
| `power` | Power | Powers |
| `talent` | Talent | Talents |

These render when a system supplies no facets — which is every currently-shipped
pack until one opts in. A pack is never required to supply facets.

## Frontend resolver contract

New `apps/web/src/utils/abilityFacets.ts`, modeled directly on the existing
`apps/web/src/utils/sizeCategory.ts`:

```ts
export type AbilityClassificationKey = "spell" | "feat" | "power" | "talent";

export interface AbilityFacetEntry {
  label: string;
  pluralLabel?: string;
}

export type AbilityFacetsLookup = Record<string, AbilityFacetEntry>;

export const DEFAULT_ABILITY_FACETS: Record<AbilityClassificationKey, Required<AbilityFacetEntry>>;

/** Singular label for one classification. Falls back to the built-in default
 *  for a missing table, missing key, non-object entry, or empty label. */
export function resolveAbilityLabel(
  lookup: AbilityFacetsLookup | undefined,
  classification: AbilityClassificationKey,
): string;

/** Plural label. Falls back to the entry's own `label`, then to the built-in
 *  default plural. Never derives a plural by appending "s". */
export function resolveAbilityPluralLabel(
  lookup: AbilityFacetsLookup | undefined,
  classification: AbilityClassificationKey,
): string;
```

**Design rules, inherited verbatim from `sizeCategory.ts`'s doc comment:**

- Stays system-agnostic — never imports any specific system pack.
- Any future pack shipping the same shape gets facet labels for free.
- Every lookup is total: it always returns a usable string, never throws, never
  returns `undefined`.

## Runtime path

```text
packs/systems/<id>/system.json  ("abilityFacets")
  → GET /api/systems/<id>/manifest.json   (returns manifest verbatim — no server change)
  → getGameSystemManifest(systemId)       (apps/web/src/api/gameSystems.ts)
  → manifest.abilityFacets as AbilityFacetsLookup | undefined
  → resolveAbilityLabel(lookup, classification)
  → rendered label
```

Component consumption mirrors `TokenPanel.tsx`'s `sizeCategories` effect: a
`useEffect` keyed on the world's `gameSystemId`, an `active` cancellation flag,
and `.catch(() => setFacets(undefined))` so a manifest fetch failure degrades to
default labels rather than breaking the view.

`SystemManifest` (declared in `apps/web/src/contexts/GameSystemContext.tsx`)
carries a `[key: string]: any` index signature, so `manifest.abilityFacets` is
already type-accessible without widening the type.

## Where labels MUST appear (FR-012)

Every user-facing surface showing a classification:

- the Compendium Abilities tab's table column and any grouping/filter control
- the ability preview panel
- the ability detail page (view and edit)
- the classification picker in the create/edit form
- an actor's known-abilities list
- the shared-ability read-only preview page

## Invariants

- **Display-only.** Facets never affect stored data. Changing a world's system
  changes only labels; `world_abilities.classification` values are untouched
  (FR-013, SC-006).
- **Portable.** Ability data authored under one system remains valid and
  viewable under any other, because the underlying classification set is fixed
  and shared (FR-009).
- **Not extensible.** A system may re-label the four classifications but cannot
  add a fifth (Non-Goals). Unknown keys in `abilityFacets` are ignored.

## Test expectations

- `resolveAbilityLabel(undefined, "spell")` → `"Spell"`.
- `resolveAbilityLabel({}, "spell")` → `"Spell"`.
- `resolveAbilityLabel({ spell: { label: "Scroll" } }, "spell")` → `"Scroll"`.
- `resolveAbilityLabel({ spell: { label: "  " } }, "spell")` → `"Spell"` (empty
  label falls back).
- `resolveAbilityLabel({ spell: "Scroll" as any }, "spell")` → `"Spell"`
  (non-object entry falls back).
- `resolveAbilityPluralLabel({ spell: { label: "Scroll" } }, "spell")` →
  `"Scroll"` (falls back to `label`, **not** `"Scrolls"`).
- A classification with a facet and one without render correctly side by side.
- Two classifications given identical labels still present as distinct choices
  in the authoring picker (spec Edge Cases).
