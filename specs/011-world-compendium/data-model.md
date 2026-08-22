# Data Model: World Compendium

## Modified Entity: World

One new nullable column on the existing `worlds` table.

| Field | Type | Notes |
|---|---|---|
| `session_notes` | `TEXT`, nullable | The DM's latest freeform between-sessions recap. `NULL`/empty both mean "no notes yet" for display purposes (FR-013 treats an explicit empty save as valid, distinct from "never set" only insofar as the row now has a saved-but-empty value rather than `NULL` — the UI does not need to distinguish the two, both render the same "No notes yet" empty state). No length cap beyond the column's natural `TEXT` limit. |

No other columns, indexes, or constraints change on `worlds`.

## Reused Entity: Actor (`world_actors`, spec 010 — unchanged)

The Compendium's NPCs tab reads and writes this entity exactly as spec 010 already defined it (`id`, `world_id`, `scene_id`, `actor_type`, `game_system_id`, `label`, `description`, `is_public`, `is_npc`, `created_by`, `owned_by`, `my_permission_level` (computed)). No fields, constraints, or permission semantics are added or changed by this feature — see `specs/010-world-staging-actors/data-model.md` for the full definition.

## New Frontend-Only Concept: Compendium Tab

Not a persisted entity — a plain in-memory descriptor used to drive the tabbed shell:

```text
CompendiumTabDef = {
  value: string;       // e.g. "npcs", "items", "abilities"
  label: string;        // e.g. "NPCs"
  icon?: string;         // fantasy-icon name, matches existing Tabs usage
  content: ReactNode;    // <NpcCompendiumTab /> | <ComingSoonTab label="Items" /> | ...
}
```

Adding a future real tab means adding one entry to this array plus its content component — no schema, routing, or preview-panel change (research.md §4).

## State: Selected Actor (Compendium NPCs tab, client-side only)

`WorldCompendiumPage` holds `selectedActorId: string | null` in component state. Selecting a table row sets it; the `ActorPreviewPanel` looks up the corresponding `WorldActorRecord` from the already-fetched roster array (research.md §3) and renders it, or renders nothing/a placeholder when `null`. This state is not persisted anywhere (not in the URL, not server-side) — reloading the page or navigating away clears the selection, consistent with every other transient UI-selection state elsewhere in the app (e.g. the canvas's own token selection).
