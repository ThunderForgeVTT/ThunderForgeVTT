# Quickstart: GM Staging Page and Full-Screen Play Canvas

Validates the feature end-to-end against a live dev stack. Assumes `make dev` (or the equivalent manual `docker compose up -d postgres rustfs` + migrations + `pnpm dev`) is already running.

## Prerequisites

- A registered account with at least one world it owns (GM/Owner), containing at least one scene.
- A second registered account invited into that same world as a `Player` (via the existing invite-code flow), for User Story 3.
- (Optional, for a non-empty NPC roster) At least one `world_actors` row with `is_npc = true` for that world — can be inserted directly via SQL for this manual walkthrough, since no NPC-creation UI is in scope for this spec:
  ```sql
  insert into world_actors (id, world_id, scene_id, actor_type, label, created_by, owned_by, is_public, is_npc, created_at, updated_at)
  values (gen_random_uuid(), '<world_id>', '<scene_id>', 'npc', 'Goblin Scout', '<owner_user_id>', '<owner_user_id>', false, true, now(), now());
  ```

## Scenario 1 — GM sees a real staging page, not the old placeholder shell (US1, FR-001/FR-002/FR-004)

1. As the GM, navigate to `/world/:id/play` for the owned world.
2. **Expected**: a staging page loads — not the canvas, not the old `WorldLayout.tsx` panels with lorem-ipsum text, and no dead "Return to dashboard" link. Sections show: the world's real scene(s) (via the existing scene switcher), the world's real member list (at least the GM), and an NPC roster reflecting real `world_actors` rows (or a genuine empty state if none exist).

## Scenario 2 — "Play" enters full-screen canvas mode (US1, FR-006/FR-007)

1. From the staging page, click "Play".
2. **Expected**: the Bevy canvas takes over the full viewport. No permanent sidebar panel, no cramped column layout. Only small on-screen controls are visible (back-to-staging, sidebar toggle).
3. If the engine/scene are still loading at this moment, the existing spec-008 loading indicators ("Downloading engine…" / "Starting engine…" / "Loading scene…") still appear as before, now inside the full-screen canvas.

## Scenario 3 — Sidebar exposes tools without permanently losing canvas space (US2, FR-009/FR-011)

1. In full-screen canvas mode, trigger the sidebar toggle.
2. **Expected**: the sidebar opens, showing scene switching (same scene switcher as staging), an NPC/combat section (same roster data as staging), and a trackers/settings section. A lore section appears as a clearly-labeled placeholder/extension point, not real content.
3. Collapse the sidebar. **Expected**: canvas returns to occupying the full viewport.
4. Exercise at least one existing canvas tool (e.g. the wall tool) while in full-screen mode. **Expected**: works exactly as it does today (unchanged canvas-tool behavior — this feature only changes the surrounding chrome).

## Scenario 4 — Back-to-staging preserves engine/canvas state (US1, FR-008)

1. While in full-screen canvas mode with the engine already loaded, click the on-screen "back to setup" control.
2. **Expected**: returns to the staging page without a full page reload.
3. Click "Play" again.
4. **Expected**: the canvas reappears **instantly** — no repeat of the "Downloading engine…" loading sequence, since the engine was never unmounted/re-initialized (research.md §1).

## Scenario 5 — Player gets the same shell, read-only (US3, FR-012/FR-013)

1. As the invited Player account, navigate to the same world's `/world/:id/play`.
2. **Expected**: the same staging page appears, showing the real scene/member/NPC data, but with no controls to create/edit scenes or edit the NPC roster.
3. Click "Play". **Expected**: enters the same full-screen canvas mode independently — this must not affect what the GM (or any other connected player) currently sees, and vice versa (open both accounts in separate browser sessions to confirm neither navigation affects the other).
4. As a third check, using a session with no membership in this world at all, attempt to navigate to `/world/:id/play`. **Expected**: the real staging data is not shown (same visibility enforcement as `scenes`/`worldMembers` today).

## Success Criteria Mapping

| Scenario | Validates |
|---|---|
| 1 | SC-001, SC-003 |
| 2 | SC-001, SC-002 |
| 3 | SC-004, SC-005 |
| 4 | SC-002 (implicitly — no repeated load) |
| 5 | SC-001 (role-correct), FR-013 |
