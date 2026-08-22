# Feature Specification: GM Staging Page and Full-Screen Play Canvas

**Feature Branch**: `009-gm-staging-page`

**Created**: 2026-08-21

**Status**: Draft

**Input**: User description: "big bug loading the play scene ... loads a huge white space and blocks the scene configurations... maybe we could get more creative with the play section by having a landing page before the play canvas where the gm can configure players / lore (not yet planned) / npcs etc and then have a big Play button where the canvas takes over the entire screen and going back becomes a on screen element back to setup but the onscreen elements become like a sidebar of scenes, npc combat, trackers, lore, settings (more later maybe)" — grounded in a code-level audit of `apps/web/src/layouts/world-layout/WorldLayout.tsx`, which today is a leftover placeholder shell used for every visit to `/world/:id/play`: a header with a dead "Return to dashboard" link (points at the unrelated `/counter` demo page), a permanent left sidebar whose two panels contain only unbuilt placeholder text ("Future world metadata... can mount in this sidebar", "Actor sheets, permissions, and compendium panels can sit beside the scene"), and the actual Bevy canvas crammed into a `lg:2.2fr` column alongside all of it — which is what reads as a "huge white space blocking scene configuration."

## Clarifications

### Session 2026-08-21

- Q: When the GM clicks "Play" on the staging page, does the transition to full-screen canvas mode happen only in the GM's own browser (each person navigates independently, at their own pace), or does it also push every currently-connected player into full-screen canvas mode at the same moment? → A: Per-user only — clicking "Play" changes what the GM sees in their own browser. Players are not forced into any view by the GM's navigation; each person's screen reflects their own choice of staging vs. canvas.
- Q: In this spec, do players get their own staging page before the canvas (with a reduced, read-only view of scenes/session info), or do they skip straight to full-screen canvas mode, since staging as described is GM configuration? → A: Players get the same staging-page-first flow as the GM, minus editing rights — they see the current scene/session info and a "Play" button of their own, but every configuration control (scene creation, NPC/roster editing) is disabled or hidden for non-GM roles.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - GM configures a session before handing the screen to the game (Priority: P1)

A GM who owns or runs a world opens `/world/:id/play` and, instead of being dropped straight into a cramped, half-broken layout with the canvas squeezed into a corner, lands on a staging page: a real, single-purpose screen that lets them see and adjust what matters before a session starts — which scene will be active, who's currently in the world (from existing membership data), and the world's NPC roster — with a clear extension point (not built in this pass) for lore. When they're ready, one prominent "Play" action takes them into a full-screen canvas.

**Why this priority**: This directly replaces the currently-broken/placeholder experience every GM hits today at the single most important screen in the product (the actual game session). Nothing else in this spec matters if this page doesn't exist or doesn't work.

**Independent Test**: As a world's GM/owner, navigate to `/world/:id/play`; confirm a staging page appears (not the canvas) showing real scene, player, and NPC data for that world, with no placeholder/lorem-ipsum content and no dead links; click "Play" and confirm the canvas takes over the full viewport.

**Acceptance Scenarios**:

1. **Given** a GM navigates to a world's play URL, **When** the page loads, **Then** they see a staging page (not the canvas) with a scene selector reflecting the world's real scenes, a player list reflecting the world's real membership, and an NPC roster reflecting the world's real NPC actors — no placeholder text standing in for unbuilt functionality.
2. **Given** a GM is on the staging page, **When** they click the "Play" action, **Then** the Bevy canvas takes over the entire viewport — no cramped column, no permanent sidebar chrome, no dead "Return to dashboard" link.
3. **Given** a GM is in full-screen canvas mode, **When** they use the on-screen control to go back, **Then** they return to the staging page, and the world's canvas state (camera, selection, in-progress edits) is not lost or corrupted by the round trip.

---

### User Story 2 - GM keeps essential tools available without losing screen space to the canvas (Priority: P1)

Once in full-screen canvas mode, a GM running a live session still needs quick access to scene switching, NPC/combat tracking, and other trackers/settings — but not at the cost of permanently losing a third of the screen to a sidebar, which is the core complaint about today's layout.

**Why this priority**: Equally load-bearing as US1 — a full-screen canvas that provides no way to switch scenes or manage the session mid-play would just move the current problem instead of solving it.

**Independent Test**: In full-screen canvas mode, trigger the on-screen sidebar toggle; confirm it reveals scene switching, NPC/combat, and trackers/settings sections without permanently reducing the canvas's viewport when collapsed.

**Acceptance Scenarios**:

1. **Given** a GM is in full-screen canvas mode with the sidebar collapsed, **When** they view the canvas, **Then** it occupies the full viewport with only small, unobtrusive on-screen controls (back-to-staging, sidebar toggle) visible.
2. **Given** a GM opens the on-screen sidebar, **When** it's open, **Then** it exposes scene switching (reusing the existing scene switcher), an NPC/combat section, and a trackers/settings section, without requiring a page navigation or losing canvas state.
3. **Given** a GM is using an existing canvas authoring tool (wall, lighting, shape, map import, asset paste, or token tool) in full-screen mode, **When** they interact with it, **Then** it continues to work exactly as it does today — this feature changes the chrome around the canvas, not the canvas tools themselves.

---

### User Story 3 - Players get a matching, read-only staging experience (Priority: P2)

A player (non-GM world member) who opens the same world's play URL should not see GM-only editing controls, but should still get the benefit of a real staging page — confirming what scene/session they're joining — before choosing to enter the full-screen canvas themselves, on their own schedule.

**Why this priority**: Extends the same fix to the majority of a world's users (players usually outnumber the GM), but is independently less urgent than the GM's own experience, since the GM is the one who was most acutely blocked by today's layout.

**Independent Test**: As a non-GM world member, navigate to `/world/:id/play`; confirm the staging page appears with scene/session info visible but all GM-only editing controls (scene creation, NPC roster editing) disabled or absent; click "Play" and confirm entry into the same full-screen canvas mode.

**Acceptance Scenarios**:

1. **Given** a player (non-GM member) navigates to a world's play URL, **When** the staging page loads, **Then** they see the current scene and session/player info, but cannot create or edit scenes, and cannot edit the NPC roster.
2. **Given** a player is on the staging page, **When** they click "Play", **Then** they enter full-screen canvas mode independently of what the GM or other players are currently viewing — no player is forced into or out of the canvas by another user's navigation.

---

### Edge Cases

- What happens when a world has zero NPC actors yet? The staging page's NPC section shows a real empty state (not a placeholder), consistent with how the existing scene switcher already handles a world with no scenes.
- What happens when a non-member (someone with a valid session but not in `world_members` and not the world's owner) hits `/world/:id/play` directly? They must not see the staging page's real data — this reuses the same visibility/ownership rule already enforced elsewhere for world access (see `require_world_member`/`load_visible_world_by_id`).
- What happens when a GM removes the scene currently open in another user's full-screen canvas view? Out of scope for this spec to solve fully — existing scene-load-error handling in `WorldPage.tsx` already covers a scene disappearing/failing to load; this spec does not need new handling beyond what's already there.
- What happens on a very small viewport (e.g. a narrow window)? The staging page and the collapsed full-screen canvas controls must remain usable — no acceptance criterion requires a dedicated mobile layout, but nothing should become totally inaccessible.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST show a staging page — not the canvas — as the first thing any world member sees at `/world/:id/play`, replacing today's `WorldLayout.tsx` placeholder shell entirely.
- **FR-002**: The staging page MUST show the world's real scenes (reusing existing scene data/`SceneSwitcher`), the world's real members (reusing existing `worldMembers` data), and the world's real NPC roster.
- **FR-003**: The system MUST expose a way to list a world's NPC actors (`world_actors` rows where `is_npc` is true) to the staging page — this data exists today but has no read API; this spec closes that specific, minimal gap.
- **FR-004**: The staging page MUST NOT show placeholder or lorem-ipsum-style content standing in for a real feature; a section with no data yet (e.g. zero NPCs) MUST show a genuine empty state instead.
- **FR-005**: The staging page MUST include a clearly-labeled extension point for a future "lore" section, without building lore content, editing, or storage in this pass.
- **FR-006**: The staging page MUST provide a single, prominent "Play" action that transitions the current user (and only the current user) into full-screen canvas mode.
- **FR-007**: In full-screen canvas mode, the Bevy canvas MUST occupy the entire viewport, replacing the permanent placeholder sidebar and cramped canvas column from today's layout.
- **FR-008**: Full-screen canvas mode MUST provide an on-screen control to return to the staging page, without a full page reload and without losing in-progress canvas/world-sync state.
- **FR-009**: Full-screen canvas mode MUST provide an on-screen, toggleable sidebar exposing: scene switching (reusing the existing scene switcher), an NPC/combat section, and a trackers/settings section. This sidebar MUST be collapsible so the canvas can occupy the full viewport when it's closed.
- **FR-010**: The toggleable sidebar MUST include a clearly-labeled extension point for a future "lore" section, without building lore content in this pass.
- **FR-011**: All existing canvas authoring tools (wall, lighting, shape, map import, asset paste, token) and the existing token panel MUST continue to function unchanged inside full-screen canvas mode.
- **FR-012**: Non-GM world members (players) MUST see the same staging-page-first and full-screen-canvas flow as the GM, with scene-creation and NPC-roster-editing controls disabled or hidden for their role.
- **FR-013**: A user who is not a member of the world (and not its owner) MUST NOT see the staging page's real scene/player/NPC data, consistent with existing world-visibility enforcement used elsewhere.
- **FR-014**: Transitioning between the staging page and full-screen canvas mode is a per-user, per-browser navigation choice — it MUST NOT be synchronized across users (a GM entering or leaving the canvas MUST NOT force any other connected user's view to change).
- **FR-015**: Player-character assignment (which world member controls which token/actor) is explicitly OUT OF SCOPE for this spec; the staging page's player list may show membership/role information but MUST NOT attempt to build character-assignment UI or data model changes here.

### Key Entities *(include if feature involves data)*

- **World Actor (existing, `world_actors` table)**: Represents both NPCs and player characters in a world/scene, distinguished by the existing `is_npc` flag; this spec only adds a read path (list NPC actors for a world) — no schema change.
- **World Member (existing, `world_members` table)**: Represents a user's role (`Owner`/`GM`/`Player`) within a world; already fully queryable via the existing `worldMembers` GraphQL query — reused as-is for the staging page's player list.
- **Scene (existing)**: Already fully queryable via existing scene APIs and `SceneSwitcher` — reused as-is for the staging page's scene section and the full-screen sidebar's scene section.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of visits to a world's play URL show either a real staging page or a full-screen canvas — never the old placeholder-panel layout, and never a blank/white screen with no explanation.
- **SC-002**: A GM can go from landing on the staging page to a full-screen, usable canvas in one click ("Play"), with no intermediate forced steps.
- **SC-003**: Every control visible on the staging page and in the full-screen sidebar reflects real data or a genuine empty state — zero placeholder/lorem-ipsum text remains in the reachable UI for this feature.
- **SC-004**: A GM can access scene switching, NPC/combat info, and trackers/settings from full-screen canvas mode without leaving that mode, and can collapse that access back down to a fully unobstructed canvas view.
- **SC-005**: All existing canvas-authoring end-to-end test coverage (wall, lighting, shape, map import, asset paste, token tools) continues to pass unmodified in intent — this feature changes the surrounding chrome, not the tools' behavior.

## Assumptions

- The staging page is reached at the same route as today (`/world/:id/play`); the staging-vs-full-screen distinction is a UI state within that route, not a new URL, so existing links/tests that navigate to `/world/:id/play` remain valid entry points.
- "GM" for authorization purposes means the existing `Owner`/`GM` roles already enforced elsewhere (e.g. `world_invites` query's Owner/GM check); no new role is introduced.
- Lore is explicitly out of scope for both the staging page and the full-screen sidebar in this pass — both need only a clearly-labeled placeholder/extension point, not any real content, editing, or storage.
- Player-character assignment via the invite/membership system (letting a GM designate which world member controls which token) is explicitly out of scope and is expected to be a fast-follow spec once this staging/full-screen shell exists to host it.
- "NPC/combat" and "trackers" in the full-screen sidebar are UI sections that this spec must surface with real, non-placeholder NPC roster data (per FR-003); actual combat-tracker mechanics (initiative order, turn tracking, etc.) beyond listing the NPC roster are not specified here and may be a further fast-follow.
- No new backend data model is required beyond the one minimal read-API gap identified (listing a world's NPC `world_actors`) — scenes, members, and canvas/world-sync state all already have working read/write paths this feature reuses.
