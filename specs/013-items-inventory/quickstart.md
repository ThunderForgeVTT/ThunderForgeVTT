# Quickstart: Validating the Items & Inventory System

Prerequisites: local dev stack running (`docker compose up` for Postgres, server on its configured port, `apps/web` dev server running), at least two user accounts in the same world (one DM/Owner, one Player), at least one existing Actor in that world, and — if spec 012 has landed — at least one existing lore entry for the correlation checks.

## US1 — DM authors an Item with a description and structured effects

1. As the world's DM, open `/world/<id>/compendium`, select the "Items" tab, and create a new Item named "Potion of Healing" with a description.
2. Add a `heal` effect: formula `3d6`, target `Hit Points`. Save. **Expect**: the item appears in the Item catalog; reopening it shows the effect exactly as authored.
3. Create a second Item, "Longsword." Add an `attack-roll` effect (formula `1d20 + STAT + MODIFIERS`, target e.g. `Attack Roll`) and a separate `damage` effect (formula `2d8`, target `Hit Points`). Save. **Expect**: both effects persist independently and both display on reopen, in authored order.
4. Remove the `damage` effect from Longsword and re-save. **Expect**: only the `attack-roll` effect remains; reopening confirms it.
5. Attempt to save an effect with an empty formula. **Expect**: rejected with a clear validation error, nothing persisted (FR-006).

## US2 — Actors hold Items in a quantity-based inventory

1. As an Editor/Owner of an Actor, open that Actor's sheet and add "Potion of Healing" to its inventory with quantity 3. **Expect**: the inventory list shows quantity 3.
2. Add "Potion of Healing" again with quantity 2. **Expect**: the existing entry merges to quantity 5 — no duplicate row is created (SC-002).
3. Adjust the quantity down to 0. **Expect**: the entry disappears from the inventory list (FR-011), not shown as a zero-quantity row.
4. Add "Longsword" with quantity 1. As a user with only Viewer access to this Actor, view the inventory. **Expect**: the Longsword entry is visible but no add/remove/adjust controls are present.
5. As a user with no access at all to this Actor, attempt to view its inventory. **Expect**: access denied.

## US3 — Items referenced from Lore the same way Actors are (requires spec 012)

1. As the DM, editing a lore entry, type `[[Potion of Healing]]`. **Expect**: the autocomplete offers the Item (alongside any same-titled lore entries/actors as distinct, disambiguated choices, per FR-016). Save.
2. **Expect**: the rendered link navigates to the Item's detail page.
3. Open the Item's detail page. **Expect**: the lore entry appears in its "linked from" list.
4. Delete the Item. **Expect**: the delete succeeds without being blocked by the outstanding lore link (FR-017). Reopen the lore entry. **Expect**: the link now renders as unresolved/broken rather than pointing at nothing or crashing the render.

## US4 — Any world member browses the Item catalog read-only

1. As a Player (non-DM), open the Compendium's Items tab. **Expect**: the same searchable table and row-preview as the DM sees, populated with real items.
2. Search by a partial item name. **Expect**: the table filters as-you-type.
3. **Expect**: no "Add Item" control is visible; an "Edit" action appears on a previewed item only if the Player's effective permission on that item is Editor or Owner.

## US5 — Share an Item and copy it into another world

1. As a member with Owner-level access to an Item (or the DM), generate a share link for it.
2. Open the link as a different, unrelated logged-in user (in a different world or no shared world). **Expect**: a read-only preview (name, description, icon, effects) with no edit controls or ownership-block visibility.
3. Click "Copy to World," pick one of the viewer's own DM-level worlds, and confirm. **Expect**: a new, fully independent Item (with cloned effects) appears in that world's Item catalog.
4. Edit the copy's description. **Expect**: the original Item is unaffected, and vice versa.
5. Revoke the share link. Attempt to open it again. **Expect**: a clear "no longer available" state.

## Cross-cutting checks

- **Name collision nudge**: as the DM, start creating a new Item whose name closely matches an existing Item's name. **Expect**: a non-blocking "did you mean [existing item]?" hint appears; saving with the duplicate name anyway succeeds (FR-019/FR-020 — never a hard block).
- **Optional icon**: create an Item with no icon/image. **Expect**: it saves successfully and displays with a placeholder/empty icon state; add an icon afterward and confirm it updates.
- **Deleted-item inventory row**: with an Item held in an Actor's inventory, delete the Item from a different surface (e.g. the Compendium). Reopen the Actor's inventory. **Expect**: the row remains, clearly marked as referencing a deleted item (using its last-known name), rather than silently vanishing.
- **Inventory permission is Actor-scoped, not Item-scoped**: as a user with Editor access to an Actor but only Viewer access to a specific Item, add that Item to the Actor's inventory. **Expect**: succeeds (FR-013/Assumptions — inventory permission follows the Actor, not the Item).
