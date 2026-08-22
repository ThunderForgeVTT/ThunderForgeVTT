# Quickstart: Validating World Compendium

Prerequisites: local dev stack running (server + `apps/web` dev server), one DM/Owner account for "World A" with at least one NPC already in the roster, one Player account joined to World A.

## US1 — DM browses and manages NPCs in the Compendium

1. As World A's DM, navigate to `/world/<id>/compendium`. **Expect**: standard app header/navigation (not full-screen canvas); a tabbed layout with NPCs selected by default.
2. Confirm the NPCs tab shows the real roster (name + description per row), matching what previously appeared on Session Setup.
3. Type into the search field. **Expect**: the table narrows to matching rows instantly (no full-page reload, no visible network round trip per keystroke).
4. Click a row. **Expect**: a preview panel appears docked to the right of the table showing that NPC's name, description, classification, and type; the table stays visible/scrollable.
5. In the preview panel, click "Edit" (assuming DM-level access). **Expect**: navigation to `/world/<id>/actor/<actorId>/edit`.
6. Return to the Compendium, use the "Add NPC" control. **Expect**: the new NPC appears in the table without a full page reload.

## US2 — Player browses the Compendium read-only

1. As the Player, navigate to `/world/<id>/compendium`. **Expect**: same NPCs tab, table, and search as the DM sees, populated with real data.
2. Select a row. **Expect**: the same preview panel content as the DM would see for that row.
3. Confirm the "Add NPC" control is absent.
4. Select an NPC the Player has only default-Viewer access to. **Expect**: the preview panel shows a "View" action but no "Edit" action.

## US3 — Session Setup is simplified

1. As either the DM or the Player, load `/world/<id>/staging`. **Expect**: exactly three sections — Play, Players, Last Session Notes. No NPC list, no "Lore — coming soon" placeholder.
2. Confirm a clearly-labeled link/button to the Compendium is present.
3. As the DM, type into Last Session Notes and save. **Expect**: the save succeeds without a full page reload.
4. Reload the page (as the DM, then separately as the Player). **Expect**: both see the updated notes text; the Player's copy is read-only (no Save control visible).
5. As the DM, clear the notes entirely and save (empty string). **Expect**: the save succeeds; reloading shows the "No notes yet" empty state, not an error.

## Placeholder tabs

1. On the Compendium, click the Items tab. **Expect**: a clearly-labeled "coming soon" message — no table, no search box, no fabricated rows.
2. Repeat for the Abilities tab.

## Full regression check

- Confirm `/world/:id/actor/:id/view` and `/edit` (spec 010) are unaffected — reachable both from the Compendium's preview panel and by direct URL as before.
- Confirm the NPC catalog's underlying capability (search-as-you-type, add-NPC, permission-gated edit) is unchanged in behavior, only its surface (Session Setup → Compendium) moved.
- Confirm a non-member of World A is rejected from `/world/<id>/compendium`, matching the existing behavior for `/world/<id>/staging` and `/world/<id>/play`.
- Run existing spec 010 e2e coverage (`world-staging-route.spec.ts`, `actor-ownership.spec.ts`, `actor-share.spec.ts`, `actor-detail-routes.spec.ts`) — none of it should reference the now-removed staging NPC panel in a way that breaks; update any that do.
