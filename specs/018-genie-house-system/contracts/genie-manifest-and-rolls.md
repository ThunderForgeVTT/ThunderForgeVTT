# Contract: Genie Manifest, Rolls, and Scene Topology

Genie introduces almost no new API surface — nearly everything routes through contracts that already exist from prior specs. This file documents exactly what's new versus what's reused.

## `system.json` manifest (new content, existing contract)

Uses the `SystemManifest` / `SystemManifestLegal` shape already implemented in `crates/pack_system_spec` (spec 016) — no contract change. New Genie-specific top-level manifest keys (not part of the shared contract, following the same precedent as `dnd5e`'s own `spellSlots` field being system-specific):

```json
{
  "id": "genie",
  "title": "Genie",
  "legal": {
    "licenseName": "ThunderForgeVTT Original Content",
    "attributionText": "Genie is an original system created for ThunderForgeVTT. No external attribution required.",
    "requiredNotice": null,
    "disclaimer": null,
    "trademarkRestrictions": [],
    "requiredUiPlacement": null,
    "sourceUrl": null
  },
  "abilities": { "...": "..." },
  "skills": { "...": "..." },
  "wishPoints": { "1": [2], "2": [3], "...": "..." },
  "sizeCategories": {
    "diminutive": { "scale": 0.5, "label": "Diminutive" },
    "small": { "scale": 0.75, "label": "Small" },
    "medium": { "scale": 1.0, "label": "Medium" },
    "large": { "scale": 2.0, "label": "Large" },
    "huge": { "scale": 3.0, "label": "Huge" },
    "colossal": { "scale": 4.0, "label": "Colossal" }
  },
  "conditions": [{ "key": "bound", "label": "Bound", "description": "..." }],
  "manifestationRoll": {
    "formula": "{skill}d6k{keep}!6cs>=4",
    "description": "..."
  },
  "data_types": {
    "character_data_types": { "...": "ability_data / resource_data / proficiency_data / condition_data" },
    "npc_data_types": { "...": "ability_data / resource_data / size_category" }
  }
}
```

Validated by a Genie-specific validator (`packs/systems/genie/server/src/validators.rs`), mirroring `packs/systems/dnd5e/server/src/validators.rs`'s existing pattern — no changes to the shared `SystemManifest` contract itself.

## Manifestation roll (existing `rollDice` mutation, spec 014)

```graphql
rollDice(input: {
  worldId: ID!,
  formula: "{skill}d6k{keep}!6cs>=4",
  placeholders: [{ name: "skill", value: 4.0 }, { name: "keep", value: 3.0 }]
}): GraphQLRollRecord!
```

No contract change to `rollDice` itself (contracts/graphql-roll.md, spec 014) — Genie is simply a caller that exercises `ResolutionKind::SuccessCount` with a formula requiring keep/drop and exploding at once, which the existing contract already supports per research.md R3.

## Scene topology (existing scene mutations, one new permitted value)

```graphql
updateScene(id: ID!, input: { gridType: "gridless", ... }): Scene!
```

No new mutation. `gridType` already accepts a string in the existing scene create/update contract; the only change is what the server-side CHECK constraint (data-model.md) and any server-side enum validation permit — `"gridless"` becomes a valid value alongside `"square"`/`"hex"`.

## Items and lore (existing contracts, spec 013 / spec 012 — no changes)

Wish-Granted Items use the existing `addWorldItem` / `addItemEffect` mutations (spec 013) verbatim; the Patron/Lineage link uses the existing `world_lore_entries` read path (spec 012) verbatim, referenced by UUID from within a Genie character's actor system data.

## Engine-side contract: `plugins/grid.rs` gridless interaction

Not a network contract — an internal Bevy system contract. The existing `match scene.grid_type { GridType::Gridless => (), ... }` arm (`src/engine/src/plugins/grid.rs`) gains a real body: token placement/movement in a gridless scene bypasses grid-snapping entirely (free-form positioning, consistent with how Cypher System's own abstract range bands were described as "no fixed measurement" in the licensed digests that inspired this feature). No new Bevy events or resources are introduced beyond what `SceneData`/`GridType` already expose.
