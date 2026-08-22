# Quickstart: Validating World Staging Route and Actor Ownership

Prerequisites: local dev stack running (`docker compose up` for Postgres/RustFS, server on its configured port, `apps/web` dev server running), at least two user accounts (one who will act as DM/Owner of "World A," one who will act as a Player in "World A" and separately as DM/Owner of their own "World B").

## US1 — DM lands on staging, catalogs NPCs, plays

1. As World A's DM, sign in and open `/welcome`.
2. Click "Enter" on World A. **Expect**: land on `/world/<id>/staging`, standard app header visible (not full-screen canvas).
3. Confirm the actor roster shows real actors (or a genuine empty state if none exist yet) — no placeholder text.
4. Use the "add NPC" control, provide a name, confirm. **Expect**: the new NPC appears in the roster without a page reload.
5. Click "Play." **Expect**: navigation to `/world/<id>/play`, full-screen canvas, no staging chrome.

## US2 — Player lands on the same route, read-only

1. As a Player member of World A, open `/welcome` and click "Enter" on World A.
2. **Expect**: land on `/world/<id>/staging`; no "add NPC" or other DM-only control visible.
3. Confirm the roster is visible (read-only) and click "Play." **Expect**: same full-screen canvas as the DM sees, independently of the DM's own navigation.

## US3 — DM manages an actor's ownership block

1. As World A's DM, from the staging roster, open any actor's `/world/<id>/actor/<actorId>/view`, then its `/edit` route.
2. Open the ownership block. **Expect**: every World A member plus the DM listed, each showing an explicit level or "default (Viewer)."
3. Assign the Player "Owner" on a PC actor. **Expect**: that player's `myPermissionLevel` for that actor becomes `OWNER` on next fetch (verify via the Player account: they can now edit that actor and — in a live session — control its token).
4. As the Player (not DM) who now holds Owner, attempt to reach the ownership-block UI for that same actor. **Expect**: no ownership-block controls shown/editable.
5. As the DM, assign a second Player "Owner" on the same actor (multiple simultaneous Owners). **Expect**: both players show `OWNER`; both can control the actor's token in a live session (last action wins — no error from either).

## US4 — Dedicated actor routes

1. As a member with at least Viewer access, navigate directly to `/world/<id>/actor/<actorId>/view`. **Expect**: renders with real data.
2. As a member with only Viewer access, navigate directly to `/world/<id>/actor/<actorId>/edit`. **Expect**: redirected to the `/view` route, no edit form shown.
3. As a non-member of World A, attempt either route. **Expect**: denied, consistent with existing world-visibility rules (same behavior as attempting `/world/<id>` as a non-member).

## US5 — Share an actor and copy it to another world

1. As World A's DM (Owner-level on the actor), open an actor's detail screen and generate a share link.
2. Open that link in a different browser/session as an unrelated user who is DM of their own "World B" but has no relationship to World A. **Expect**: a read-only preview of the actor — no edit controls, no ownership-block visibility, no indication of which world it came from.
3. Click "Copy to World." **Expect**: a picker listing only worlds where this user holds DM-level access (including World B); confirm World B.
4. **Expect**: a new, independent actor appears in World B's staging roster, with cloned abilities/items/lore data, and a clear success notification.
5. Edit the copy's label in World B, then re-check the original actor in World A. **Expect**: the original is unchanged. Edit the original in World A; **expect**: the World B copy is unaffected.
6. As the DM back in World A, revoke the share link. Attempt to open the same link again (any user). **Expect**: "no longer available" state, not the actor's data.
7. Attempt "Copy to World" as a user who is not DM of any world. **Expect**: the picker shows no eligible destinations and explains why.

## Full regression check

- Confirm `/world/:id/play` reached directly (bookmarked, no prior `/staging` visit) still renders the full-screen canvas immediately — no forced redirect to `/staging`, per the spec's migration note.
- Confirm `/world/:id` (the existing world dashboard/`CampaignSettingsPanel`) is unchanged by this feature.
- Run existing canvas-authoring e2e coverage (`apps/web/e2e/`) unmodified — this feature's changes to `WorldPage.tsx` (removing the `playView` state) must not regress wall/lighting/shape/map-import/asset-paste/token tool behavior once in full-screen mode.
